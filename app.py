import json
import logging
import os
import re
from pathlib import Path
import sys
from functools import wraps
from urllib.parse import urlencode

import requests
from flask import Flask, flash, jsonify, redirect, render_template, request, session, url_for
from dotenv import load_dotenv
from werkzeug.security import check_password_hash, generate_password_hash

load_dotenv()

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s',
    handlers=[logging.StreamHandler(sys.stdout)]
)

app = Flask(__name__)
app.secret_key = 'your_secret_key'  # Change this

API_BASE_URL = os.getenv('API_BASE_URL')
API_TOKEN = os.getenv('API_TOKEN')
USERS_FILE = Path('users.json')


def normalize_username(username: str) -> str:
    return username.strip().lower()


def _write_users(data):
    USERS_FILE.write_text(json.dumps(data, indent=2))


def _initialize_users():
    if not USERS_FILE.exists():
        default_owner = {
            'owner': {
                'password': generate_password_hash('owner123'),
                'role': 'owner',
                'assigned_instances': []
            }
        }
        _write_users(default_owner)
    with USERS_FILE.open('r') as f:
        raw_data = json.load(f)
    normalized = {}
    changed = False
    for username, payload in raw_data.items():
        key = normalize_username(username)
        if key != username:
            changed = True
        payload.setdefault('role', 'admin')
        payload.setdefault('assigned_instances', [])
        normalized[key] = payload
    if changed:
        _write_users(normalized)
    return normalized


users = _initialize_users()


def save_users():
    _write_users(users)


def get_current_user():
    username = session.get('username')
    if not username:
        return None
    user = users.get(username)
    if not user:
        return None
    data = user.copy()
    data['username'] = username
    return data


def resolve_default_endpoint(user):
    if not user:
        return 'login'
    return 'create_start' if user.get('role') == 'owner' else 'instances'


def login_required(view):
    @wraps(view)
    def wrapped(*args, **kwargs):
        if 'username' not in session:
            return redirect(url_for('login'))
        return view(*args, **kwargs)

    return wrapped


def owner_required(view):
    @wraps(view)
    def wrapped(*args, **kwargs):
        if 'username' not in session:
            return redirect(url_for('login'))
        user = get_current_user()
        if user and user.get('role') == 'owner':
            return view(*args, **kwargs)
        flash('Owner permissions are required for this area.')
        return redirect(url_for('instances'))

    return wrapped


def can_access_instance(instance_id: str) -> bool:
    user = get_current_user()
    if not user:
        return False
    if user.get('role') == 'owner':
        return True
    assigned = set(str(i) for i in user.get('assigned_instances', []))
    return str(instance_id) in assigned


def enforce_instance_access(instance_id):
    if can_access_instance(instance_id):
        return None
    flash('You do not have access to this instance.')
    return redirect(url_for('instances'))


def api_call(method, endpoint, data=None, params=None):
    url = f"{API_BASE_URL}{endpoint}"
    headers = {'API-Token': API_TOKEN}
    logging.info(f"API Request: {method} {url} - Params: {params} - Data: {data}")
    if method == 'GET':
        response = requests.get(url, headers=headers, params=params)
    elif method == 'POST':
        response = requests.post(url, headers=headers, json=data)
    elif method == 'DELETE':
        response = requests.delete(url, headers=headers)
    else:
        raise ValueError('Unsupported HTTP method')
    try:
        payload = response.json()
    except ValueError:
        payload = {'raw': response.text}
    logging.info(
        "API Response: %s %s - Status: %s - Body: %s",
        method,
        url,
        response.status_code,
        payload
    )
    return payload


def tidy_numeric(value):
    try:
        numeric = float(value)
    except (TypeError, ValueError):
        return ''
    if float(numeric).is_integer():
        return str(int(numeric))
    return f"{numeric:.1f}".rstrip('0').rstrip('.')


def format_spec_value(raw, unit=None):
    if raw is None:
        return ''
    if isinstance(raw, (int, float)):
        display = tidy_numeric(raw)
        return f"{display} {unit}".strip() if unit else display
    text = str(raw).strip()
    if not text:
        return ''
    if unit and re.fullmatch(r'\d+(?:\.\d+)?', text):
        return f"{text} {unit}".strip()
    return text


def parse_metric(raw, unit=None):
    if raw is None:
        return None
    if isinstance(raw, (int, float)):
        numeric = float(raw)
        display = tidy_numeric(numeric)
        label = f"{display} {unit}".strip() if unit else display
        return {'display': display, 'label': label, 'numeric': numeric}
    text = str(raw).strip()
    if not text:
        return None
    match = re.search(r'\d+(?:\.\d+)?', text)
    numeric = float(match.group(0)) if match else None
    display = tidy_numeric(numeric) if numeric is not None else text
    if unit:
        if unit.lower() in text.lower():
            label = text
        else:
            label = f"{display} {unit}".strip()
    else:
        label = text
    return {'display': display, 'label': label.strip(), 'numeric': numeric}


def extract_spec_value(spec, keys):
    if not spec:
        return None
    for key in keys:
        if key not in spec:
            continue
        value = spec[key]
        if value in (None, ''):
            continue
        if key == 'ramInMb':
            try:
                return float(value) / 1024
            except (TypeError, ValueError):
                return value
        return value
    return None


def join_tags(tags):
    if not tags:
        return ''
    if isinstance(tags, str):
        return tags
    if isinstance(tags, (list, tuple)):
        return ', '.join(str(tag) for tag in tags if tag not in (None, ''))
    return str(tags)


def parse_flag(value, default=False):
    if value is None:
        return default
    text = str(value).strip().lower()
    if not text:
        return default
    return text in {'1', 'true', 'yes', 'on'}


def flag_to_str(value):
    return '1' if bool(value) else '0'


def parse_int_list(raw_values):
    result = []
    for raw in raw_values:
        text = str(raw).strip()
        if not text:
            continue
        try:
            result.append(int(text))
        except ValueError:
            continue
    return result


def parse_optional_int(value):
    if value is None:
        return None
    text = str(value).strip()
    if text == '':
        return None
    try:
        return int(text)
    except ValueError:
        return None


def parse_wizard_base(source):
    hostnames = []
    for entry in source.getlist('hostnames'):
        cleaned = entry.strip()
        if cleaned:
            hostnames.append(cleaned)
    region = (source.get('region') or '').strip()
    instance_class = (source.get('instance_class') or 'default').strip() or 'default'
    plan_type = (source.get('plan_type') or 'fixed').strip().lower()
    if plan_type not in {'fixed', 'custom'}:
        plan_type = 'fixed'
    assign_ipv4 = parse_flag(source.get('assign_ipv4'), default=True)
    assign_ipv6 = parse_flag(source.get('assign_ipv6'), default=False)
    floating_ip_count = parse_optional_int(source.get('floating_ip_count'))
    if floating_ip_count is None:
        floating_ip_count = 0
    ssh_key_ids = parse_int_list(source.getlist('ssh_key_ids'))

    return {
        'hostnames': hostnames,
        'region': region,
        'instance_class': instance_class,
        'plan_type': plan_type,
        'assign_ipv4': assign_ipv4,
        'assign_ipv6': assign_ipv6,
        'floating_ip_count': floating_ip_count,
        'ssh_key_ids': ssh_key_ids,
    }


def build_base_query_pairs(state):
    pairs = []
    for hostname in state.get('hostnames', []):
        pairs.append(('hostnames', hostname))
    if state.get('region'):
        pairs.append(('region', state['region']))
    pairs.append(('instance_class', state.get('instance_class', 'default')))
    pairs.append(('plan_type', state.get('plan_type', 'fixed')))
    pairs.append(('assign_ipv4', flag_to_str(state.get('assign_ipv4'))))
    pairs.append(('assign_ipv6', flag_to_str(state.get('assign_ipv6'))))
    floating = state.get('floating_ip_count')
    if floating is not None:
        pairs.append(('floating_ip_count', str(floating)))
    for key_id in state.get('ssh_key_ids', []):
        pairs.append(('ssh_key_ids', str(key_id)))
    return pairs


def build_query_string(pairs):
    query = urlencode(pairs, doseq=True)
    return query


def load_regions():
    response = api_call('GET', '/v1/regions')
    raw_regions = response.get('data', []) if response.get('code') == 'OKAY' else []
    active = [region for region in raw_regions if region.get('isActive')]
    regions = active or raw_regions
    lookup = {region.get('id'): region for region in regions if region.get('id')}
    configs = {identifier: region.get('config', {}) or {} for identifier, region in lookup.items()}
    return regions, lookup, configs


def load_ssh_keys():
    response = api_call('GET', '/v1/ssh-keys')
    payload = response.get('data', []) if response.get('code') == 'OKAY' else []
    if isinstance(payload, dict):
        candidates = (
            payload.get('sshKeys')
            or payload.get('ssh_keys')
            or payload.get('items')
            or payload.get('data')
            or []
        )
    else:
        candidates = payload

    normalized = []
    for item in candidates or []:
        if not isinstance(item, dict):
            continue
        key_id = item.get('id')
        normalized.append({
            'id': key_id,
            'name': item.get('name') or (f"SSH Key {key_id}" if key_id is not None else 'SSH Key'),
            'public_key': item.get('publicKey') or item.get('public_key') or '',
            'fingerprint': item.get('fingerprint') or item.get('fingerPrint') or '',
            'customer_id': item.get('customerId') or item.get('userId') or item.get('customer_id'),
        })
    return normalized


def is_high_frequency(product):
    plan = product.get('plan') or {}
    plan_name = (plan.get('name') or '').lower()
    tags = product.get('tags')
    if isinstance(tags, list):
        tags_text = ' '.join(str(tag) for tag in tags)
    else:
        tags_text = str(tags or '')
    tags_text = tags_text.lower()
    instance_class = str(product.get('instanceClass') or plan.get('instanceClass') or '').lower()
    return 'high frequency' in plan_name or 'high frequency' in tags_text or instance_class == 'cpu-optimized'


def format_price(value, cadence):
    try:
        numeric = float(value)
    except (TypeError, ValueError):
        return None
    return f"${numeric:.2f} / {cadence}"


def build_product_view(product, region_lookup):
    plan = product.get('plan') or {}
    spec = plan.get('specification') or {}
    region_id = product.get('regionId') or (product.get('region') or {}).get('id')
    region_meta = region_lookup.get(region_id) if region_lookup else None
    region_name = region_meta.get('name') if region_meta else plan.get('name') or str(product.get('id'))

    spec_entries = []

    cpu_value = extract_spec_value(spec, ['cpu', 'vCpu', 'vcpu', 'cores'])
    if cpu_value is not None:
        spec_entries.append({'term': 'CPU', 'value': format_spec_value(cpu_value, 'vCPU')})

    ram_value = extract_spec_value(spec, ['ram', 'ramInGB', 'ramInMb', 'memory'])
    if ram_value is not None:
        spec_entries.append({'term': 'RAM', 'value': format_spec_value(ram_value, 'GB')})

    disk_value = extract_spec_value(spec, ['disk', 'diskInGB', 'storage'])
    if disk_value is not None:
        spec_entries.append({'term': 'Disk', 'value': format_spec_value(disk_value, 'GB')})

    bandwidth_value = extract_spec_value(spec, ['bandwidth', 'bandwidthInTB'])
    if bandwidth_value is not None:
        spec_entries.append({'term': 'Bandwidth', 'value': format_spec_value(bandwidth_value, 'TB')})

    gpu_value = extract_spec_value(spec, ['gpuCount', 'gpu', 'gpus'])
    gpu_metric = parse_metric(gpu_value, 'GPU') if gpu_value is not None else None

    metrics = []
    metrics.append({
        'type': 'region',
        'value': region_name,
        'label': f"Region {region_name}",
        'icon': 'region.svg',
        'variant': None,
    })

    if gpu_metric and gpu_metric['display'] not in ('', '0'):
        plural = ''
        if gpu_metric['numeric'] not in (None, 1):
            plural = 's'
        metrics.append({
            'type': 'gpu',
            'value': gpu_metric['display'],
            'label': gpu_metric['label'] if 'GPU' in gpu_metric['label'].upper() else f"{gpu_metric['label']} GPU{plural}",
            'icon': 'gpu.svg',
            'variant': None,
        })

    cpu_metric = parse_metric(cpu_value, 'vCPU') if cpu_value is not None else None
    if cpu_metric:
        metrics.append({
            'type': 'cpu',
            'value': cpu_metric['display'],
            'label': cpu_metric['label'],
            'icon': 'cpu.svg',
            'variant': 'hf' if is_high_frequency(product) else None,
        })

    ram_metric = parse_metric(ram_value, 'GB') if ram_value is not None else None
    if ram_metric:
        label = ram_metric['label']
        if 'ram' not in label.lower():
            label = f"{label} RAM"
        metrics.append({
            'type': 'ram',
            'value': ram_metric['display'],
            'label': label,
            'icon': 'ram.svg',
            'variant': None,
        })

    price_items = product.get('priceItems') or []
    price_entry = price_items[0] if price_items else {}
    price_entries = []
    hourly = format_price(price_entry.get('hourlyPrice'), 'hr')
    monthly = format_price(price_entry.get('monthlyPrice'), 'mo')
    if hourly:
        price_entries.append({'term': 'Hourly', 'value': hourly})
    if monthly:
        price_entries.append({'term': 'Monthly', 'value': monthly})

    tags_text = join_tags(product.get('tags'))

    return {
        'id': str(product.get('id')),
        'plan_name': plan.get('name'),
        'description': plan.get('description'),
        'metrics': metrics,
        'spec_entries': spec_entries,
        'price_entries': price_entries,
        'tags': tags_text,
        'region_id': region_id,
        'region_name': region_name,
        'raw': product,
    }



@app.context_processor
def inject_user():
    return {'current_user': get_current_user()}


@app.route('/login', methods=['GET', 'POST'])
def login():
    current = get_current_user()
    if current:
        return redirect(url_for(resolve_default_endpoint(current)))
    if request.method == 'POST':
        username = normalize_username(request.form['username'])
        password = request.form['password']
        user = users.get(username)
        if user and check_password_hash(user['password'], password):
            session['username'] = username
            flash('Welcome back!')
            current = get_current_user()
            return redirect(url_for(resolve_default_endpoint(current)))
        flash('Invalid credentials')
    return render_template('login.html')


@app.route('/logout', methods=['POST'])
@login_required
def logout():
    session.pop('username', None)
    flash('See you soon!')
    return redirect(url_for('login'))


@app.route('/users', methods=['GET', 'POST'])
@owner_required
def manage_users():
    if request.method == 'POST':
        username = normalize_username(request.form['username'])
        password = request.form['password']
        role = request.form['role']
        if not username or not password:
            flash('Username and password are required.')
            return redirect(url_for('manage_users'))
        if role not in {'owner', 'admin'}:
            flash('Invalid role selection.')
            return redirect(url_for('manage_users'))
        if username in users:
            flash('Username already exists.')
            return redirect(url_for('manage_users'))
        users[username] = {
            'password': generate_password_hash(password),
            'role': role,
            'assigned_instances': []
        }
        save_users()
        flash('User created successfully.')
        return redirect(url_for('manage_users'))
    return render_template('users.html', users=users)


@app.route('/users/<username>/reset-password', methods=['POST'])
@owner_required
def reset_password(username):
    target = users.get(username)
    if not target:
        flash('User not found.')
        return redirect(url_for('manage_users'))
    new_password = request.form['new_password']
    if not new_password:
        flash('Password cannot be empty.')
        return redirect(url_for('manage_users'))
    target['password'] = generate_password_hash(new_password)
    save_users()
    flash('Password updated.')
    return redirect(url_for('manage_users'))


@app.route('/users/<username>/role', methods=['POST'])
@owner_required
def update_role(username):
    target = users.get(username)
    if not target:
        flash('User not found.')
        return redirect(url_for('manage_users'))
    if username == session.get('username'):
        flash('You cannot change your own role.')
        return redirect(url_for('manage_users'))
    new_role = request.form['role']
    if new_role not in {'owner', 'admin'}:
        flash('Invalid role selection.')
        return redirect(url_for('manage_users'))
    if target.get('role') == 'owner' and new_role != 'owner':
        remaining = [u for u in users if users[u].get('role') == 'owner' and u != username]
        if not remaining:
            flash('At least one owner is required.')
            return redirect(url_for('manage_users'))
    target['role'] = new_role
    if new_role == 'owner':
        target['assigned_instances'] = []
    else:
        target.setdefault('assigned_instances', [])
    save_users()
    flash('Role updated.')
    return redirect(url_for('manage_users'))


@app.route('/users/<username>/delete', methods=['POST'])
@owner_required
def delete_user(username):
    if username not in users:
        flash('User not found.')
        return redirect(url_for('manage_users'))
    if username == session.get('username'):
        flash('You cannot delete the currently logged-in account.')
        return redirect(url_for('manage_users'))
    if users[username].get('role') == 'owner':
        remaining = [u for u in users if users[u].get('role') == 'owner' and u != username]
        if not remaining:
            flash('At least one owner is required.')
            return redirect(url_for('manage_users'))
    del users[username]
    save_users()
    flash('User removed.')
    return redirect(url_for('manage_users'))


@app.route('/access')
@owner_required
def access_management():
    response = api_call('GET', '/v1/instances')
    instances = response.get('data', {}).get('instances', []) if response.get('code') == 'OKAY' else []
    instance_map = {str(instance['id']): instance for instance in instances}
    admins = {u: data for u, data in users.items() if data.get('role') == 'admin'}
    return render_template('access.html', admins=admins, instance_map=instance_map)


@app.route('/access/<username>', methods=['POST'])
@owner_required
def update_access(username):
    target = users.get(username)
    if not target or target.get('role') != 'admin':
        flash('Admin not found.')
        return redirect(url_for('access_management'))
    selected_instances = request.form.getlist('instances')
    normalized = [inst.strip() for inst in selected_instances if inst.strip()]
    target['assigned_instances'] = normalized
    save_users()
    flash('Assignments updated.')
    return redirect(url_for('access_management'))


@app.route('/ssh-keys', methods=['GET', 'POST'])
@owner_required
def ssh_keys():
    form_values = {
        'name': '',
        'public_key': '',
    }

    if request.method == 'POST':
        action = (request.form.get('action') or 'create').strip().lower()

        if action == 'delete':
            key_id_raw = (request.form.get('ssh_key_id') or '').strip()
            if not key_id_raw.isdigit():
                flash('Invalid SSH key identifier provided.')
            else:
                response = api_call('DELETE', f"/v1/ssh-keys/{key_id_raw}")
                if response.get('code') == 'OKAY':
                    flash('SSH key removed.')
                else:
                    detail = response.get('detail') or 'Unable to delete SSH key.'
                    flash(f'Error: {detail}')
            return redirect(url_for('ssh_keys'))

        name = (request.form.get('name') or '').strip()
        public_key = (request.form.get('public_key') or '').strip()
        form_values.update({'name': name, 'public_key': public_key})

        errors = []
        if not name:
            errors.append('Provide a name for the SSH key.')
        if not public_key:
            errors.append('Provide the public key material.')

        if errors:
            for message in errors:
                flash(message)
        else:
            payload = {'name': name, 'publicKey': public_key}
            response = api_call('POST', '/v1/ssh-keys', data=payload)
            if response.get('code') == 'OKAY':
                flash('SSH key added successfully.')
                return redirect(url_for('ssh_keys'))
            detail = response.get('detail') or 'Unable to add SSH key.'
            flash(f'Error: {detail}')

    ssh_keys_list = load_ssh_keys()
    return render_template('ssh_keys.html', ssh_keys=ssh_keys_list, form_values=form_values)


@app.route('/')
def root():
    user = get_current_user()
    if user:
        return redirect(url_for(resolve_default_endpoint(user)))
    return redirect(url_for('login'))


@app.route('/instances')
@login_required
def instances():
    response = api_call('GET', '/v1/instances')
    if response.get('code') == 'OKAY':
        instances = response['data']['instances']
    else:
        instances = []
        flash('Error fetching instances')
    user = get_current_user()
    if user and user.get('role') == 'admin':
        allowed_ids = set(str(i) for i in user.get('assigned_instances', []))
        instances = [instance for instance in instances if str(instance['id']) in allowed_ids]
    return render_template('instances.html', instances=instances)


@app.route('/create', methods=['GET', 'POST'])
@owner_required
def create_start():
    if request.args.get('reset') == '1':
        return redirect(url_for('create_start'))

    regions, region_lookup, _ = load_regions()
    if not regions:
        flash('No regions were returned by the developer API.')
    ssh_keys = load_ssh_keys()
    available_ssh_key_ids = {str(key['id']) for key in ssh_keys if key.get('id') is not None}

    base_state = parse_wizard_base(request.args)
    if not base_state['region'] and regions:
        base_state['region'] = regions[0].get('id')

    form_data = {
        'hostnames': ', '.join(base_state['hostnames']),
        'region': base_state['region'] or '',
        'instance_class': base_state['instance_class'],
        'assign_ipv4': base_state['assign_ipv4'],
        'assign_ipv6': base_state['assign_ipv6'],
        'plan_type': base_state['plan_type'],
        'floating_ip_count': str(base_state['floating_ip_count']),
        'selected_ssh_keys': [str(item) for item in base_state['ssh_key_ids']],
    }

    if request.method == 'POST':
        hostnames_raw = request.form.get('hostnames', '')
        hostnames = [h.strip() for h in hostnames_raw.split(',') if h.strip()]
        selected_region = request.form.get('region', '').strip()
        instance_class = request.form.get('instance_class', 'default').strip() or 'default'
        plan_type = request.form.get('plan_type', 'fixed')
        if plan_type not in {'fixed', 'custom'}:
            plan_type = 'fixed'
        assign_ipv4 = 'assign_ipv4' in request.form
        assign_ipv6 = 'assign_ipv6' in request.form
        floating_ip_raw = request.form.get('floating_ip_count', '').strip()
        selected_key_values = [value.strip() for value in request.form.getlist('ssh_key_ids') if value.strip()]

        errors = []

        if not hostnames:
            errors.append('Provide at least one hostname (comma-separated).')
        if len(hostnames) > 10:
            errors.append('You can provision at most ten hostnames per request.')
        if not selected_region or (region_lookup and selected_region not in region_lookup):
            errors.append('Select a deployment region.')

        floating_ip_count = 0
        if floating_ip_raw:
            try:
                floating_ip_count = int(floating_ip_raw)
            except ValueError:
                errors.append('Floating IP count must be a number between 0 and 5.')
                floating_ip_count = 0
            else:
                if floating_ip_count < 0 or floating_ip_count > 5:
                    errors.append('Floating IP count must be a number between 0 and 5.')

        ssh_key_ids = []
        for token in selected_key_values:
            if not token.isdigit():
                errors.append('Invalid SSH key selection detected.')
                ssh_key_ids = []
                break
            if available_ssh_key_ids and token not in available_ssh_key_ids:
                errors.append('Selected SSH key is no longer available.')
                ssh_key_ids = []
                break
            ssh_key_ids.append(int(token))

        if errors:
            for message in errors:
                flash(message)
        else:
            base_state = {
                'hostnames': hostnames,
                'region': selected_region,
                'instance_class': instance_class,
                'plan_type': plan_type,
                'assign_ipv4': assign_ipv4,
                'assign_ipv6': assign_ipv6,
                'floating_ip_count': floating_ip_count,
                'ssh_key_ids': ssh_key_ids,
            }
            query_pairs = build_base_query_pairs(base_state)
            query_string = build_query_string(query_pairs)
            target = url_for('create_fixed') if plan_type == 'fixed' else url_for('create_custom')
            if query_string:
                target = f"{target}?{query_string}"
            return redirect(target)

        form_data.update({
            'hostnames': hostnames_raw,
            'region': selected_region or base_state['region'] or form_data['region'],
            'instance_class': instance_class,
            'plan_type': plan_type,
            'assign_ipv4': assign_ipv4,
            'assign_ipv6': assign_ipv6,
            'floating_ip_count': floating_ip_raw if floating_ip_raw else '0',
            'selected_ssh_keys': selected_key_values,
        })

    if regions and not form_data['region']:
        form_data['region'] = regions[0]['id']

    return render_template(
        'create/start.html',
        regions=regions,
        form_data=form_data,
        ssh_keys=ssh_keys,
    )


@app.route('/create/fixed', methods=['GET', 'POST'])
@owner_required
def create_fixed():
    source = request.form if request.method == 'POST' else request.args
    base_state = parse_wizard_base(source)
    base_pairs = build_base_query_pairs(base_state)

    if not base_state['hostnames'] or not base_state['region']:
        flash('Start the wizard from step 1.')
        query = build_query_string(base_pairs)
        target = url_for('create_start')
        if query:
            target = f"{target}?{query}"
        return redirect(target)

    if base_state['plan_type'] != 'fixed':
        query = build_query_string(base_pairs)
        if base_state['plan_type'] == 'custom':
            target = url_for('create_custom')
            if query:
                target = f"{target}?{query}"
            return redirect(target)
        flash('Choose a plan type before selecting a product.')
        target = url_for('create_start')
        if query:
            target = f"{target}?{query}"
        return redirect(target)

    regions, region_lookup, _ = load_regions()
    region = region_lookup.get(base_state['region'])
    if not region:
        flash('Selected region is no longer available.')
        query = build_query_string(base_pairs)
        target = url_for('create_start')
        if query:
            target = f"{target}?{query}"
        return redirect(target)

    products_response = api_call('GET', '/v1/products', params={'regionId': region.get('id')})
    products_raw = products_response.get('data', []) if products_response.get('code') == 'OKAY' else []
    products = [build_product_view(item, region_lookup) for item in products_raw]
    product_ids = {product['id'] for product in products}

    selected_product_id = source.get('product_id', '').strip()
    extras_defaults = {
        'extra_disk': source.get('extra_disk', '0').strip() or '0',
        'extra_bandwidth': source.get('extra_bandwidth', '0').strip() or '0',
    }

    if request.method == 'POST':
        selected_product_id = request.form.get('product_id', '').strip()
        disk_raw = request.form.get('extra_disk', '0').strip()
        bandwidth_raw = request.form.get('extra_bandwidth', '0').strip()

        errors = []
        if not products:
            errors.append('No products are available for this region.')
        if not selected_product_id:
            errors.append('Select a product before continuing.')
        elif selected_product_id not in product_ids:
            errors.append('Selected product is no longer available for this region.')

        def parse_non_negative(raw_value, label):
            if raw_value == '':
                return 0
            try:
                value = int(raw_value)
            except ValueError:
                errors.append(f'{label} must be a non-negative number.')
                return 0
            if value < 0:
                errors.append(f'{label} must be a non-negative number.')
                return 0
            return value

        extra_disk = parse_non_negative(disk_raw, 'Extra disk (GB)')
        extra_bandwidth = parse_non_negative(bandwidth_raw, 'Extra bandwidth (TB)')

        extras_defaults['extra_disk'] = disk_raw or '0'
        extras_defaults['extra_bandwidth'] = bandwidth_raw or '0'

        if errors:
            for message in errors:
                flash(message)
        else:
            query_pairs = base_pairs + [
                ('product_id', selected_product_id),
                ('extra_disk', str(extra_disk)),
                ('extra_bandwidth', str(extra_bandwidth)),
            ]
            query = build_query_string(query_pairs)
            target = url_for('create_review')
            if query:
                target = f"{target}?{query}"
            return redirect(target)

    back_query = build_query_string(base_pairs)
    back_to_start_url = url_for('create_start')
    if back_query:
        back_to_start_url = f"{back_to_start_url}?{back_query}"

    return render_template(
        'create/fixed.html',
        region=region,
        products=products,
        selected_product_id=selected_product_id,
        extras=extras_defaults,
        base_state=base_state,
        back_to_start_url=back_to_start_url,
    )


@app.route('/create/custom', methods=['GET', 'POST'])
@owner_required
def create_custom():
    source = request.form if request.method == 'POST' else request.args
    base_state = parse_wizard_base(source)
    base_pairs = build_base_query_pairs(base_state)

    if not base_state['hostnames'] or not base_state['region']:
        flash('Start the wizard from step 1.')
        query = build_query_string(base_pairs)
        target = url_for('create_start')
        if query:
            target = f"{target}?{query}"
        return redirect(target)

    if base_state['plan_type'] != 'custom':
        query = build_query_string(base_pairs)
        if base_state['plan_type'] == 'fixed':
            target = url_for('create_fixed')
            if query:
                target = f"{target}?{query}"
            return redirect(target)
        flash('Choose a plan type before defining custom resources.')
        target = url_for('create_start')
        if query:
            target = f"{target}?{query}"
        return redirect(target)

    regions, region_lookup, region_configs = load_regions()
    region = region_lookup.get(base_state['region'])
    if not region:
        flash('Selected region is no longer available.')
        query = build_query_string(base_pairs)
        target = url_for('create_start')
        if query:
            target = f"{target}?{query}"
        return redirect(target)

    config = region_configs.get(region.get('id'), {}) or {}

    def threshold(raw_value, fallback):
        try:
            numeric = int(raw_value)
            return max(numeric, fallback)
        except (TypeError, ValueError):
            return fallback

    minimums = {
        'ram': threshold(config.get('ramThresholdInGB'), 1),
        'disk': threshold(config.get('diskThresholdInGB'), 1),
    }
    requirements = []
    if config.get('ramThresholdInGB'):
        requirements.append(f"RAM ≥ {config['ramThresholdInGB']} GB")
    if config.get('diskThresholdInGB'):
        requirements.append(f"Disk ≥ {config['diskThresholdInGB']} GB")

    form_values = {
        'cpu': source.get('cpu', '').strip(),
        'ramInGB': source.get('ramInGB', '').strip(),
        'diskInGB': source.get('diskInGB', '').strip(),
        'bandwidthInTB': source.get('bandwidthInTB', '').strip(),
    }

    if request.method == 'POST':
        errors = []

        def parse_required_int(field, label, minimum):
            raw_value = request.form.get(field, '').strip()
            if not raw_value:
                errors.append(f'{label} is required.')
                return None
            try:
                value = int(raw_value)
            except ValueError:
                errors.append(f'{label} must be a whole number.')
                return None
            if value < minimum:
                errors.append(f'{label} must be at least {minimum}.')
                return None
            return value

        def parse_optional_int(field, label, minimum):
            raw_value = request.form.get(field, '').strip()
            if not raw_value:
                return None
            try:
                value = int(raw_value)
            except ValueError:
                errors.append(f'{label} must be a whole number.')
                return None
            if value < minimum:
                errors.append(f'{label} must be at least {minimum}.')
                return None
            return value

        cpu_value = parse_required_int('cpu', 'CPU', 1)
        ram_value = parse_required_int('ramInGB', 'RAM (GB)', minimums['ram'])
        disk_value = parse_required_int('diskInGB', 'Disk (GB)', minimums['disk'])
        bandwidth_value = parse_optional_int('bandwidthInTB', 'Bandwidth (TB)', 1)

        form_values.update({
            'cpu': request.form.get('cpu', '').strip(),
            'ramInGB': request.form.get('ramInGB', '').strip(),
            'diskInGB': request.form.get('diskInGB', '').strip(),
            'bandwidthInTB': request.form.get('bandwidthInTB', '').strip(),
        })

        if errors:
            for message in errors:
                flash(message)
        else:
            query_pairs = base_pairs + [
                ('cpu', str(cpu_value)),
                ('ramInGB', str(ram_value)),
                ('diskInGB', str(disk_value)),
            ]
            if bandwidth_value is not None:
                query_pairs.append(('bandwidthInTB', str(bandwidth_value)))
            query = build_query_string(query_pairs)
            target = url_for('create_review')
            if query:
                target = f"{target}?{query}"
            return redirect(target)

    back_query = build_query_string(base_pairs)
    back_to_start_url = url_for('create_start')
    if back_query:
        back_to_start_url = f"{back_to_start_url}?{back_query}"

    return render_template(
        'create/custom.html',
        region=region,
        requirements=requirements,
        minimums=minimums,
        form_values=form_values,
        base_state=base_state,
        back_to_start_url=back_to_start_url,
    )


@app.route('/create/review', methods=['GET', 'POST'])
@owner_required
def create_review():
    source = request.form if request.method == 'POST' else request.args
    base_state = parse_wizard_base(source)
    base_pairs = build_base_query_pairs(base_state)

    if not base_state['hostnames'] or not base_state['region']:
        flash('Start the wizard from step 1.')
        query = build_query_string(base_pairs)
        target = url_for('create_start')
        if query:
            target = f"{target}?{query}"
        return redirect(target)

    plan_type = base_state['plan_type']
    if plan_type not in {'fixed', 'custom'}:
        flash('Choose a plan type before reviewing.')
        query = build_query_string(base_pairs)
        target = url_for('create_start')
        if query:
            target = f"{target}?{query}"
        return redirect(target)

    regions, region_lookup, region_configs = load_regions()
    region = region_lookup.get(base_state['region'])
    if not region:
        flash('Selected region is no longer available.')
        query = build_query_string(base_pairs)
        target = url_for('create_start')
        if query:
            target = f"{target}?{query}"
        return redirect(target)

    metrics = []
    plan_summary = []
    price_entries = []
    footnote = ''
    plan_state = {}
    plan_numbers = {}
    back_target = 'create_fixed' if plan_type == 'fixed' else 'create_custom'

    if plan_type == 'fixed':
        products_response = api_call('GET', '/v1/products', params={'regionId': region.get('id')})
        products_raw = products_response.get('data', []) if products_response.get('code') == 'OKAY' else []
        product_lookup = {str(item.get('id')): item for item in products_raw}

        product_id = source.get('product_id', '').strip()
        extra_disk_raw = source.get('extra_disk', '0').strip() or '0'
        extra_bandwidth_raw = source.get('extra_bandwidth', '0').strip() or '0'
        plan_state = {
            'product_id': product_id,
            'extra_disk': extra_disk_raw,
            'extra_bandwidth': extra_bandwidth_raw,
        }

        fixed_pairs = list(base_pairs)
        fixed_pairs.extend([
            ('product_id', product_id),
            ('extra_disk', extra_disk_raw),
            ('extra_bandwidth', extra_bandwidth_raw),
        ])

        if not product_id:
            flash('Select a product before reviewing.')
            query = build_query_string(fixed_pairs)
            target = url_for('create_fixed')
            if query:
                target = f"{target}?{query}"
            return redirect(target)

        product_raw = product_lookup.get(str(product_id))
        if not product_raw:
            flash('The selected product is no longer available for this region.')
            query = build_query_string(fixed_pairs)
            target = url_for('create_fixed')
            if query:
                target = f"{target}?{query}"
            return redirect(target)

        def safe_non_negative(raw_value):
            try:
                value = int(raw_value)
            except (TypeError, ValueError):
                return 0
            return max(value, 0)

        extra_disk_value = safe_non_negative(extra_disk_raw)
        extra_bandwidth_value = safe_non_negative(extra_bandwidth_raw)
        plan_numbers = {
            'extra_disk': extra_disk_value,
            'extra_bandwidth': extra_bandwidth_value,
        }

        product_view = build_product_view(product_raw, region_lookup)
        metrics = product_view['metrics']
        plan_summary = list(product_view['spec_entries'])
        if extra_disk_value:
            plan_summary.append({'term': 'Extra Disk', 'value': f"{extra_disk_value} GB"})
        if extra_bandwidth_value:
            plan_summary.append({'term': 'Extra Bandwidth', 'value': f"{extra_bandwidth_value} TB"})
        price_entries = product_view['price_entries']
        footnote = product_view['description'] or (f"Tags: {product_view['tags']}" if product_view['tags'] else '')

        back_query = build_query_string(fixed_pairs)
    else:
        custom_raw = {
            'cpu': source.get('cpu', '').strip(),
            'ramInGB': source.get('ramInGB', '').strip(),
            'diskInGB': source.get('diskInGB', '').strip(),
            'bandwidthInTB': source.get('bandwidthInTB', '').strip(),
        }
        plan_state = custom_raw

        custom_pairs = list(base_pairs)
        custom_pairs.extend([
            ('cpu', custom_raw['cpu']),
            ('ramInGB', custom_raw['ramInGB']),
            ('diskInGB', custom_raw['diskInGB']),
        ])
        if custom_raw['bandwidthInTB']:
            custom_pairs.append(('bandwidthInTB', custom_raw['bandwidthInTB']))

        def parse_required_numeric(raw_value, label):
            if not raw_value:
                flash(f'{label} is required before reviewing.')
                query = build_query_string(custom_pairs)
                target = url_for('create_custom')
                if query:
                    target = f"{target}?{query}"
                return None, target
            try:
                value = int(raw_value)
            except ValueError:
                flash(f'{label} must be a whole number.')
                query = build_query_string(custom_pairs)
                target = url_for('create_custom')
                if query:
                    target = f"{target}?{query}"
                return None, target
            if value < 1:
                flash(f'{label} must be at least 1.')
                query = build_query_string(custom_pairs)
                target = url_for('create_custom')
                if query:
                    target = f"{target}?{query}"
                return None, target
            return value, None

        cpu_value, redirect_target = parse_required_numeric(custom_raw['cpu'], 'CPU')
        if redirect_target:
            return redirect(redirect_target)
        ram_value, redirect_target = parse_required_numeric(custom_raw['ramInGB'], 'RAM (GB)')
        if redirect_target:
            return redirect(redirect_target)
        disk_value, redirect_target = parse_required_numeric(custom_raw['diskInGB'], 'Disk (GB)')
        if redirect_target:
            return redirect(redirect_target)

        bandwidth_value = None
        if custom_raw['bandwidthInTB']:
            try:
                parsed_bandwidth = int(custom_raw['bandwidthInTB'])
            except ValueError:
                flash('Bandwidth (TB) must be a whole number.')
                query = build_query_string(custom_pairs)
                target = url_for('create_custom')
                if query:
                    target = f"{target}?{query}"
                return redirect(target)
            if parsed_bandwidth < 1:
                flash('Bandwidth (TB) must be at least 1.')
                query = build_query_string(custom_pairs)
                target = url_for('create_custom')
                if query:
                    target = f"{target}?{query}"
                return redirect(target)
            bandwidth_value = parsed_bandwidth

        plan_numbers = {
            'cpu': cpu_value,
            'ramInGB': ram_value,
            'diskInGB': disk_value,
            'bandwidthInTB': bandwidth_value,
        }

        region_name = region.get('name')
        metrics = [
            {'type': 'region', 'value': region_name, 'label': f"Region {region_name}", 'icon': 'region.svg', 'variant': None},
            {'type': 'cpu', 'value': tidy_numeric(cpu_value), 'label': f"{tidy_numeric(cpu_value)} vCPU", 'icon': 'cpu.svg', 'variant': 'hf' if base_state.get('instance_class') == 'cpu-optimized' else None},
            {'type': 'ram', 'value': tidy_numeric(ram_value), 'label': f"{tidy_numeric(ram_value)} GB RAM", 'icon': 'ram.svg', 'variant': None},
        ]
        plan_summary = [
            {'term': 'CPU', 'value': f"{tidy_numeric(cpu_value)} vCPU"},
            {'term': 'RAM', 'value': f"{tidy_numeric(ram_value)} GB"},
            {'term': 'Disk', 'value': f"{tidy_numeric(disk_value)} GB"},
        ]
        if bandwidth_value:
            plan_summary.append({'term': 'Bandwidth', 'value': f"{tidy_numeric(bandwidth_value)} TB"})
        footnote = 'Custom plan will be provisioned exactly as requested.'

        back_query = build_query_string(custom_pairs)

    back_url = url_for(back_target)
    if back_query:
        back_url = f"{back_url}?{back_query}"

    ssh_keys_catalog = load_ssh_keys()
    ssh_key_lookup = {str(item['id']): item for item in ssh_keys_catalog if item.get('id') is not None}
    ssh_keys_display = []
    for key_id in base_state['ssh_key_ids']:
        key_str = str(key_id)
        entry = ssh_key_lookup.get(key_str)
        if entry:
            label = entry.get('name') or entry.get('label') or key_str
            if entry.get('id') is not None:
                label = f"{label} (#{entry['id']})"
        else:
            label = key_str
        ssh_keys_display.append(label)

    wizard = {
        'hostnames': base_state['hostnames'],
        'plan_type': plan_type,
        'floating_ip_count': base_state['floating_ip_count'],
        'assign_ipv4': base_state['assign_ipv4'],
        'assign_ipv6': base_state['assign_ipv6'],
        'ssh_key_ids': [str(item) for item in base_state['ssh_key_ids']],
        'ssh_keys_display': ssh_keys_display,
        'instance_class': base_state['instance_class'],
        'region': base_state['region'],
    }

    if request.method == 'POST':
        payload = {
            'hostnames': base_state['hostnames'],
            'region': base_state['region'],
            'class': base_state.get('instance_class', 'default'),
            'assignIpv4': bool(base_state.get('assign_ipv4', False)),
            'assignIpv6': bool(base_state.get('assign_ipv6', False)),
        }
        floating_ip_count = base_state.get('floating_ip_count')
        if floating_ip_count is not None:
            payload['floatingIPCount'] = floating_ip_count
        ssh_key_ids = base_state.get('ssh_key_ids') or []
        if ssh_key_ids:
            payload['sshKeyIds'] = ssh_key_ids

        if plan_type == 'fixed':
            product_id = plan_state.get('product_id')
            if not product_id:
                flash('Select a product before reviewing.')
                return redirect(back_url)
            payload['productId'] = product_id
            extras = {}
            if plan_numbers.get('extra_disk'):
                extras['diskInGB'] = plan_numbers['extra_disk']
            if plan_numbers.get('extra_bandwidth'):
                extras['bandwidthInTB'] = plan_numbers['extra_bandwidth']
            if extras:
                payload['extraResource'] = extras
        else:
            extras = {
                'cpu': plan_numbers.get('cpu'),
                'ramInGB': plan_numbers.get('ramInGB'),
                'diskInGB': plan_numbers.get('diskInGB'),
            }
            if plan_numbers.get('bandwidthInTB'):
                extras['bandwidthInTB'] = plan_numbers['bandwidthInTB']
            payload['extraResource'] = extras

        response = api_call('POST', '/v1/instances', data=payload)
        if response.get('code') == 'OKAY':
            flash('Instance created successfully')
            return redirect(url_for('instances'))
        detail = response.get('detail') or 'An unknown error occurred while creating the instance.'
        flash(f"Error: {detail}")

    return render_template(
        'create/review.html',
        wizard=wizard,
        region=region,
        metrics=metrics,
        plan_summary=plan_summary,
        price_entries=price_entries,
        footnote=footnote,
        back_url=back_url,
        base_state=base_state,
        plan_state=plan_state,
    )


@app.route('/instance/<instance_id>')
@login_required
def instance_detail(instance_id):
    redirect_response = enforce_instance_access(instance_id)
    if redirect_response:
        return redirect_response
    response = api_call('GET', f'/v1/instances/{instance_id}')
    if response.get('code') == 'OKAY':
        instance = response['data']
        return render_template('instance_detail.html', instance=instance)
    flash('Instance not found')
    return redirect(url_for('instances'))


@app.route('/instance/<instance_id>/delete', methods=['POST'])
@login_required
def delete_instance(instance_id):
    redirect_response = enforce_instance_access(instance_id)
    if redirect_response:
        return redirect_response
    response = api_call('DELETE', f'/v1/instances/{instance_id}')
    if response.get('code') == 'OKAY':
        flash('Instance deleted')
    else:
        flash('Error deleting instance')
    return redirect(url_for('instances'))


@app.route('/instance/<instance_id>/poweron', methods=['POST'])
@login_required
def poweron_instance(instance_id):
    redirect_response = enforce_instance_access(instance_id)
    if redirect_response:
        return redirect_response
    response = api_call('POST', f'/v1/instances/{instance_id}/poweron')
    flash('Power on request sent' if response.get('code') == 'OKAY' else 'Error')
    return redirect(url_for('instance_detail', instance_id=instance_id))


@app.route('/instance/<instance_id>/poweroff', methods=['POST'])
@login_required
def poweroff_instance(instance_id):
    redirect_response = enforce_instance_access(instance_id)
    if redirect_response:
        return redirect_response
    response = api_call('POST', f'/v1/instances/{instance_id}/poweroff')
    flash('Power off request sent' if response.get('code') == 'OKAY' else 'Error')
    return redirect(url_for('instance_detail', instance_id=instance_id))


@app.route('/instance/<instance_id>/reset', methods=['POST'])
@login_required
def reset_instance(instance_id):
    redirect_response = enforce_instance_access(instance_id)
    if redirect_response:
        return redirect_response
    response = api_call('POST', f'/v1/instances/{instance_id}/reset')
    flash('Reset request sent' if response.get('code') == 'OKAY' else 'Error')
    return redirect(url_for('instance_detail', instance_id=instance_id))


@app.route('/instance/<instance_id>/change-pass', methods=['POST'])
@login_required
def change_pass_instance(instance_id):
    redirect_response = enforce_instance_access(instance_id)
    if redirect_response:
        return redirect_response
    response = api_call('POST', f'/v1/instances/{instance_id}/change-pass')
    if response.get('code') == 'OKAY':
        password = response['data']['password']
        flash(f'Password changed: {password}')
    else:
        flash('Error changing password')
    return redirect(url_for('instance_detail', instance_id=instance_id))


@app.route('/instance/<instance_id>/add-traffic', methods=['POST'])
@login_required
def add_traffic_instance(instance_id):
    redirect_response = enforce_instance_access(instance_id)
    if redirect_response:
        return redirect_response
    amount_raw = request.form.get('traffic_amount', '').strip()
    if not amount_raw:
        flash('Provide the amount of traffic to add.')
        return redirect(url_for('instance_detail', instance_id=instance_id))
    try:
        amount = float(amount_raw)
    except ValueError:
        flash('Traffic amount must be a number.')
        return redirect(url_for('instance_detail', instance_id=instance_id))
    if amount <= 0:
        flash('Traffic amount must be greater than zero.')
        return redirect(url_for('instance_detail', instance_id=instance_id))

    response = api_call('POST', f'/v1/instances/{instance_id}/add-traffic', data={'amount': amount})
    if response.get('code') == 'OKAY':
        flash('Traffic added successfully.')
    else:
        flash(f"Error adding traffic: {response.get('detail')}")
    return redirect(url_for('instance_detail', instance_id=instance_id))


@app.route('/instance/<instance_id>/resize', methods=['GET', 'POST'])
@login_required
def resize_instance(instance_id):
    redirect_response = enforce_instance_access(instance_id)
    if redirect_response:
        return redirect_response
    if request.method == 'POST':
        type_ = request.form['type']
        product_id = request.form.get('product_id')
        region_id = request.form.get('region_id')
        cpu = request.form.get('cpu')
        ramInGB = request.form.get('ramInGB')
        diskInGB = request.form.get('diskInGB')
        bandwidthInTB = request.form.get('bandwidthInTB')

        data = {'type': type_}
        if type_ == 'FIXED' and product_id:
            data['productId'] = product_id
        elif type_ == 'CUSTOM' and region_id:
            data['regionId'] = region_id
            if cpu or ramInGB or diskInGB or bandwidthInTB:
                data['extraResource'] = {}
                if cpu:
                    data['extraResource']['cpu'] = int(cpu)
                if ramInGB:
                    data['extraResource']['ramInGB'] = int(ramInGB)
                if diskInGB:
                    data['extraResource']['diskInGB'] = int(diskInGB)
                if bandwidthInTB:
                    data['extraResource']['bandwidthInTB'] = int(bandwidthInTB)

        response = api_call('POST', f'/v1/instances/{instance_id}/resize', data=data)
        if response.get('code') == 'OKAY':
            flash('Resize request sent')
            return redirect(url_for('instance_detail', instance_id=instance_id))
        flash(f"Error: {response.get('detail')}")
    instance_response = api_call('GET', f'/v1/instances/{instance_id}')
    instance = instance_response.get('data') if instance_response.get('code') == 'OKAY' else None
    regions_response = api_call('GET', '/v1/regions')
    regions = regions_response.get('data', []) if regions_response.get('code') == 'OKAY' else []
    return render_template('resize.html', instance=instance, regions=regions)


@app.route('/instance/<instance_id>/change-os', methods=['GET', 'POST'])
@login_required
def change_os_instance(instance_id):
    redirect_response = enforce_instance_access(instance_id)
    if redirect_response:
        return redirect_response
    if request.method == 'POST':
        os_id = request.form['os_id']
        response = api_call('POST', f'/v1/instances/{instance_id}/change-os', data={'osId': os_id})
        if response.get('code') == 'OKAY':
            flash('OS change request sent')
            return redirect(url_for('instance_detail', instance_id=instance_id))
        flash(f"Error: {response.get('detail')}")
    os_response = api_call('GET', '/v1/os')
    os_list = os_response.get('data', {}).get('os', []) if os_response.get('code') == 'OKAY' else []
    return render_template('change_os.html', instance_id=instance_id, os_list=os_list)


@app.route('/instance/<instance_id>/subscription-refund')
@login_required
def subscription_refund(instance_id):
    redirect_response = enforce_instance_access(instance_id)
    if redirect_response:
        return redirect_response
    response = api_call('GET', f'/v1/instances/{instance_id}/subscription-refund')
    if response.get('code') == 'OKAY':
        refund = response['data']
        return render_template('subscription_refund.html', refund=refund, instance_id=instance_id)
    flash('Error fetching refund details')
    return redirect(url_for('instance_detail', instance_id=instance_id))


@app.route('/bulk-subscription-refund', methods=['GET', 'POST'])
@owner_required
def bulk_subscription_refund():
    if request.method == 'POST':
        ids = request.form['ids'].split(',')
        data = {'ids': [id_.strip() for id_ in ids if id_.strip()]}
        response = api_call('POST', '/v1/instances/bulk-subscription-refund', data=data)
        if response.get('code') == 'OKAY':
            refunds = response['data']
            return render_template('bulk_refund_result.html', refunds=refunds)
        flash(f"Error: {response.get('detail')}")
    return render_template('bulk_refund.html')


@app.route('/regions')
@owner_required
def regions():
    response = api_call('GET', '/v1/regions')
    regions = response.get('data', []) if response.get('code') == 'OKAY' else []
    return render_template('regions.html', regions=regions)


@app.route('/api/regions/<region_id>/products')
@owner_required
def region_products(region_id):
    if not region_id:
        return jsonify({'products': [], 'detail': 'Region ID is required.'}), 400

    response = api_call('GET', '/v1/products', params={'regionId': region_id})
    if response.get('code') == 'OKAY':
        return jsonify({'products': response.get('data', [])})

    detail = response.get('detail') or 'Unable to load products.'
    return jsonify({'products': [], 'detail': detail}), 502


@app.route('/products')
@owner_required
def products():
    region_param = (request.args.get('region_id') or '').strip()
    regions, raw_region_lookup, _ = load_regions()
    region_lookup = {}
    for region_id, region in raw_region_lookup.items():
        if region_id is None:
            continue
        region_lookup[region_id] = region
        region_lookup[str(region_id)] = region

    selected_region = region_lookup.get(region_param)
    products = []
    if region_param:
        request_region_id = (selected_region.get('id') if selected_region else region_param)
        response = api_call('GET', '/v1/products', params={'regionId': request_region_id})
        if response.get('code') == 'OKAY':
            raw_products = response.get('data', []) or []
            products = [build_product_view(item, region_lookup) for item in raw_products]
        else:
            detail = response.get('detail') or 'Unable to load products for the selected region.'
            flash(detail)

    return render_template(
        'products.html',
        products=products,
        regions=regions,
        selected_region=selected_region,
        requested_region=region_param,
    )


@app.route('/os')
@owner_required
def os_list():
    response = api_call('GET', '/v1/os')
    os_list = response.get('data', {}).get('os', []) if response.get('code') == 'OKAY' else []
    return render_template('os.html', os_list=os_list)


@app.route('/applications')
@owner_required
def applications():
    response = api_call('GET', '/v1/applications')
    apps = response.get('data', []) if response.get('code') == 'OKAY' else []
    return render_template('applications.html', apps=apps)


if __name__ == '__main__':
    app.run(debug=True)
