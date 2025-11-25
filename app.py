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

# List of API endpoints to ignore from logging (frequent/noisy requests)
LOGGING_IGNORE_ENDPOINTS = [
    '/v1/regions',
    '/v1/products',
    '/v1/os',
    '/v1/ssh-keys',
]

app = Flask(__name__)
app.secret_key = 'your_secret_key'  # Change this

API_BASE_URL = os.getenv('API_BASE_URL')
API_TOKEN = os.getenv('API_TOKEN')
# Optional default customer id to use for API calls when the configured API token requires it
API_DEFAULT_CUSTOMER_ID = os.getenv('API_DEFAULT_CUSTOMER_ID')
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
    return 'create_step_1' if user.get('role') == 'owner' else 'instances'


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
    
    # Check if this endpoint should be logged
    should_log = endpoint not in LOGGING_IGNORE_ENDPOINTS
    
    if should_log:
        logging.info(f"API Request: {method} {url} - Params: {params} - Data: {data}")
    
    # SSL verification settings - can be disabled via environment variable for development
    verify_ssl = os.getenv('API_VERIFY_SSL', 'true').lower() == 'true'
    
    try:
        if method == 'GET':
            response = requests.get(url, headers=headers, params=params, verify=verify_ssl, timeout=30)
        elif method == 'POST':
            response = requests.post(url, headers=headers, json=data, verify=verify_ssl, timeout=30)
        elif method == 'DELETE':
            response = requests.delete(url, headers=headers, verify=verify_ssl, timeout=30)
        else:
            raise ValueError('Unsupported HTTP method')
    except requests.exceptions.SSLError as e:
        logging.error(f"SSL Error: {method} {url} - {str(e)}")
        return {'code': 'FAILED', 'detail': 'SSL connection error. Please check your connection or set API_VERIFY_SSL=false for development.', 'data': {}}
    except requests.exceptions.RequestException as e:
        logging.error(f"Request Error: {method} {url} - {str(e)}")
        return {'code': 'FAILED', 'detail': f'Network error: {str(e)}', 'data': {}}
    
    try:
        payload = response.json()
    except ValueError:
        payload = {'raw': response.text}
    
    if should_log:
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
    os_id = (source.get('os_id') or '').strip()

    return {
        'hostnames': hostnames,
        'region': region,
        'instance_class': instance_class,
        'plan_type': plan_type,
        'assign_ipv4': assign_ipv4,
        'assign_ipv6': assign_ipv6,
        'floating_ip_count': floating_ip_count,
        'ssh_key_ids': ssh_key_ids,
        'os_id': os_id,
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
    if state.get('os_id'):
        pairs.append(('os_id', state['os_id']))
    return pairs


def build_query_string(pairs):
    query = urlencode(pairs, doseq=True)
    return query


def load_regions():
    response = api_call('GET', '/v1/regions')
    raw_regions = response.get('data', []) if response.get('code') == 'OKAY' else []
    # Only return active regions - filter out inactive ones
    active = [region for region in raw_regions if region.get('isActive')]
    regions = active  # Always use active regions, even if empty
    lookup = {region.get('id'): region for region in regions if region.get('id')}
    configs = {identifier: region.get('config', {}) or {} for identifier, region in lookup.items()}
    return regions, lookup, configs


def load_os_list(min_ram_in_mb: int = None, only_actives: bool = True):
    """Load OS list from the backend with optional filters."""
    params = {'action': 'CREATE'}
    if min_ram_in_mb is not None:
        params['minRam'] = min_ram_in_mb
    if only_actives:
        params['onlyActives'] = True
    
    response = api_call('GET', '/v1/os', params=params)
    payload = response.get('data', {}).get('os', []) if response.get('code') == 'OKAY' else []
    
    normalized = []
    for item in payload or []:
        if not isinstance(item, dict):
            continue
        normalized.append({
            'id': item.get('id') or '',
            'name': item.get('name') or '',
            'family': item.get('family') or '',
            'arch': item.get('arch') or '',
            'minRam': item.get('minRam') or 0,
            'isActive': item.get('isActive', False),
            'isDefault': item.get('isDefault', False),
        })
    return normalized


def load_ssh_keys(customer_id: str = None):
    """Load SSH keys from the backend. If the API token is an admin token and the developer
    gateway requires an explicit customer id, this function will optionally use `customer_id`
    (either passed in, or via env var API_DEFAULT_CUSTOMER_ID) to retrieve the list.
    """
    params = None
    if customer_id:
        params = {'customerId': customer_id}
    response = api_call('GET', '/v1/ssh-keys', params=params)
    payload = response.get('data', []) if response.get('code') == 'OKAY' else []
    # If the API responded with an error that suggests a customer id is required for this
    # token (likely the token is an admin/developer token), try using the configured
    # API_DEFAULT_CUSTOMER_ID environment variable to request the keys for a specific
    # customer.
    if response.get('code') != 'OKAY' or isinstance(payload, dict) and not payload:
        detail = response.get('detail') or ''
        req_customer_id_needed = 'Customer id should be provided' in str(detail) or 'customer id' in str(detail).lower()
        if not customer_id and API_DEFAULT_CUSTOMER_ID and req_customer_id_needed:
            params = {'customerId': API_DEFAULT_CUSTOMER_ID}
            response = api_call('GET', '/v1/ssh-keys', params=params)
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


def determine_customer_context():
    """Return a customer_id to use for API calls.
    Precedence:
      1) request.args.get('customer_id') if present
      2) API_DEFAULT_CUSTOMER_ID environment variable
      3) None
    """
    from flask import request
    cid = (request.args.get('customer_id') or '').strip()
    if cid:
        return cid
    if API_DEFAULT_CUSTOMER_ID:
        return API_DEFAULT_CUSTOMER_ID
    return None


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
    from urllib.parse import urlparse
    api_hostname = urlparse(API_BASE_URL).netloc if API_BASE_URL else 'N/A'
    return {
        'current_user': get_current_user(),
        'api_hostname': api_hostname
    }


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

    customer_id = (request.args.get('customer_id') or determine_customer_context())
    if request.method == 'POST':
        action = (request.form.get('action') or 'create').strip().lower()

        if action == 'delete':
            key_id_raw = (request.form.get('ssh_key_id') or '').strip()
            if not key_id_raw.isdigit():
                flash('Invalid SSH key identifier provided.')
            else:
                response = api_call('DELETE', f"/v1/ssh-keys/{key_id_raw}")
                if response.get('code') != 'OKAY' and API_DEFAULT_CUSTOMER_ID and response.get('detail') and 'customer id' in response.get('detail').lower():
                    response = api_call('DELETE', f"/v1/ssh-keys/{key_id_raw}", params={'customerId': API_DEFAULT_CUSTOMER_ID})
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
            if response.get('code') != 'OKAY' and API_DEFAULT_CUSTOMER_ID and response.get('detail') and 'customer id' in response.get('detail').lower():
                payload['customerId'] = API_DEFAULT_CUSTOMER_ID
                response = api_call('POST', '/v1/ssh-keys', data=payload)
            if response.get('code') == 'OKAY':
                flash('SSH key added successfully.')
                return redirect(url_for('ssh_keys'))
            detail = response.get('detail') or 'Unable to add SSH key.'
            flash(f'Error: {detail}')

    customer_id = (request.args.get('customer_id') or determine_customer_context())
    ssh_keys_list = load_ssh_keys(customer_id=customer_id)
    return render_template('ssh_keys.html', ssh_keys=ssh_keys_list, form_values=form_values, customer_id=customer_id)


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


@app.route('/create/step-1', methods=['GET'])
@owner_required
def create_step_1():
    """Page 1: Region selection, instance class, plan type"""
    if request.args.get('reset') == '1':
        return redirect(url_for('create_step_1'))

    regions, region_lookup, _ = load_regions()
    if not regions:
        flash('No regions were returned by the developer API.')

    base_state = parse_wizard_base(request.args)
    if not base_state['region'] and regions:
        base_state['region'] = regions[0].get('id')

    form_data = {
        'region': base_state['region'] or '',
        'instance_class': base_state['instance_class'],
        'plan_type': base_state['plan_type'],
    }

    if regions and not form_data['region']:
        form_data['region'] = regions[0]['id']

    return render_template(
        'create/start.html',
        regions=regions,
        form_data=form_data,
    )


@app.route('/create/step-2', methods=['GET'])
@owner_required
def create_step_2():
    """Page 2: Hostnames and IP Assignment"""
    source = request.args
    base_state = parse_wizard_base(source)
    base_pairs = build_base_query_pairs(base_state)

    if not base_state['region']:
        flash('Please start the wizard from step 1 to select a region.')
        return redirect(url_for('create_step_1'))

    regions, region_lookup, _ = load_regions()
    region = region_lookup.get(base_state['region'])
    if not region:
        flash('Selected region is no longer available.')
        return redirect(url_for('create_step_1'))

    form_data = {
        'hostnames': ', '.join(base_state['hostnames']),
        'assign_ipv4': base_state['assign_ipv4'],
        'assign_ipv6': base_state['assign_ipv6'],
        'floating_ip_count': str(base_state['floating_ip_count']),
    }

    back_query = build_query_string(base_pairs)
    back_url = url_for('create_step_1')
    if back_query:
        back_url = f"{back_url}?{back_query}"

    return render_template(
        'create/hostnames.html',
        form_data=form_data,
        region=region,
        back_url=back_url,
        base_state=base_state,
    )


@app.route('/create/step-3', methods=['GET'])
@owner_required
def create_step_3():
    """Page 3: Resource Requirements / Product Selection"""
    source = request.args
    base_state = parse_wizard_base(source)
    base_pairs = build_base_query_pairs(base_state)

    if not base_state['hostnames'] or not base_state['region']:
        flash('Please start the wizard from step 1.')
        query = build_query_string(base_pairs)
        target = url_for('create_step_1')
        if query:
            target = f"{target}?{query}"
        return redirect(target)

    regions, region_lookup, region_configs = load_regions()
    region = region_lookup.get(base_state['region'])
    if not region:
        flash('Selected region is no longer available.')
        query = build_query_string(base_pairs)
        target = url_for('create_step_1')
        if query:
            target = f"{target}?{query}"
        return redirect(target)

    plan_type = base_state['plan_type']
    if plan_type == 'fixed':
        # Fixed plan - show products
        region_id = region.get('id')
        products_response = api_call('GET', '/v1/products', params={'regionId': region_id})
        products = products_response.get('data', []) if products_response.get('code') == 'OKAY' else []
        
        products = [build_product_view(item, region_lookup) for item in products]

        selected_product_id = source.get('product_id', '').strip()

        back_query = build_query_string(base_pairs)
        back_url = url_for('create_step_2')
        if back_query:
            back_url = f"{back_url}?{back_query}"

        return render_template(
            'create/fixed.html',
            region=region,
            products=products,
            selected_product_id=selected_product_id,
            base_state=base_state,
            back_to_start_url=back_url,
        )
    else:
        # Custom plan - show resource inputs
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

        back_query = build_query_string(base_pairs)
        back_url = url_for('create_step_2')
        if back_query:
            back_url = f"{back_url}?{back_query}"

        return render_template(
            'create/custom.html',
            region=region,
            requirements=requirements,
            minimums=minimums,
            form_values=form_values,
            base_state=base_state,
            back_to_start_url=back_url,
        )


@app.route('/create/step-4', methods=['GET'])
@owner_required
def create_step_4():
    """Page 4: Extras (Fixed Plans Only)"""
    source = request.args
    base_state = parse_wizard_base(source)
    base_pairs = build_base_query_pairs(base_state)

    if not base_state['hostnames'] or not base_state['region']:
        flash('Please start the wizard from step 1.')
        query = build_query_string(base_pairs)
        target = url_for('create_step_1')
        if query:
            target = f"{target}?{query}"
        return redirect(target)

    plan_type = base_state['plan_type']
    
    # Only fixed plans have extras - custom plans skip to step 5
    if plan_type != 'fixed':
        query = build_query_string(base_pairs)
        target = url_for('create_step_5')
        if query:
            target = f"{target}?{query}"
        return redirect(target)

    product_id = source.get('product_id', '').strip()
    if not product_id:
        flash('Select a product before continuing.')
        query = build_query_string(base_pairs)
        target = url_for('create_step_3')
        if query:
            target = f"{target}?{query}"
        return redirect(target)

    regions, region_lookup, _ = load_regions()
    region = region_lookup.get(base_state['region'])
    if not region:
        flash('Selected region is no longer available.')
        query = build_query_string(base_pairs)
        target = url_for('create_step_1')
        if query:
            target = f"{target}?{query}"
        return redirect(target)

    extras_defaults = {
        'extra_disk': source.get('extra_disk', '0').strip() or '0',
        'extra_bandwidth': source.get('extra_bandwidth', '0').strip() or '0',
    }

    back_pairs = base_pairs + [('product_id', product_id)]
    back_query = build_query_string(back_pairs)
    back_url = url_for('create_step_3')
    if back_query:
        back_url = f"{back_url}?{back_query}"

    return render_template(
        'create/extras.html',
        extras=extras_defaults,
        product_id=product_id,
        base_state=base_state,
        back_url=back_url,
    )


@app.route('/create/step-5', methods=['GET'])
@owner_required
def create_step_5():
    """Page 5: OS Selection"""
    source = request.args
    base_state = parse_wizard_base(source)
    base_pairs = build_base_query_pairs(base_state)

    if not base_state['hostnames'] or not base_state['region']:
        flash('Please start the wizard from step 1.')
        query = build_query_string(base_pairs)
        target = url_for('create_step_1')
        if query:
            target = f"{target}?{query}"
        return redirect(target)

    regions, region_lookup, _ = load_regions()
    region = region_lookup.get(base_state['region'])
    if not region:
        flash('Selected region is no longer available.')
        query = build_query_string(base_pairs)
        target = url_for('create_step_1')
        if query:
            target = f"{target}?{query}"
        return redirect(target)

    plan_type = base_state['plan_type']
    
    # Calculate RAM in MB for OS filtering
    if plan_type == 'fixed':
        product_id = source.get('product_id', '').strip()
        if not product_id:
            flash('Select a product before selecting OS.')
            query = build_query_string(base_pairs)
            target = url_for('create_step_3')
            if query:
                target = f"{target}?{query}"
            return redirect(target)
        
        products_response = api_call('GET', '/v1/products', params={'regionId': region.get('id')})
        products_raw = products_response.get('data', []) if products_response.get('code') == 'OKAY' else []
        product_lookup = {str(item.get('id')): item for item in products_raw}
        product_raw = product_lookup.get(str(product_id))
        
        if not product_raw:
            flash('Selected product is no longer available.')
            query = build_query_string(base_pairs)
            target = url_for('create_step_3')
            if query:
                target = f"{target}?{query}"
            return redirect(target)
        
        plan_spec = product_raw.get('plan', {}).get('specification', {})
        ram_in_mb = int((plan_spec.get('ram') or plan_spec.get('ramInMB') or 1024) * 1024)
    else:
        ram_value_raw = source.get('ramInGB', '').strip()
        if not ram_value_raw:
            flash('Define RAM before selecting OS.')
            query = build_query_string(base_pairs)
            target = url_for('create_step_3')
            if query:
                target = f"{target}?{query}"
            return redirect(target)
        try:
            ram_value = int(ram_value_raw)
            ram_in_mb = int(ram_value * 1024)
        except ValueError:
            flash('Invalid RAM value.')
            query = build_query_string(base_pairs)
            target = url_for('create_step_3')
            if query:
                target = f"{target}?{query}"
            return redirect(target)

    # Load OS list
    os_list = load_os_list(min_ram_in_mb=ram_in_mb, only_actives=True)
    os_id = source.get('os_id', '').strip()
    
    # Auto-select default OS if available
    if not os_id and os_list:
        default_os = next((os for os in os_list if os.get('isDefault')), None)
        if default_os:
            os_id = default_os.get('id', '')

    back_query = build_query_string(base_pairs)
    if plan_type == 'fixed':
        # Fixed plans come from step 4 (extras)
        back_url = url_for('create_step_4')
    else:
        # Custom plans come from step 3 (resource definition)
        back_url = url_for('create_step_3')
    if back_query:
        back_url = f"{back_url}?{back_query}"

    # Prepare template variables
    template_vars = {
        'os_list': os_list,
        'selected_os_id': os_id,
        'base_state': base_state,
        'back_url': back_url,
    }
    
    # Add plan-specific variables
    if plan_type == 'fixed':
        template_vars['product_id'] = source.get('product_id', '')
        template_vars['extra_disk'] = source.get('extra_disk', '0')
        template_vars['extra_bandwidth'] = source.get('extra_bandwidth', '0')
    else:
        template_vars['cpu'] = source.get('cpu', '')
        template_vars['ramInGB'] = source.get('ramInGB', '')
        template_vars['diskInGB'] = source.get('diskInGB', '')
        template_vars['bandwidthInTB'] = source.get('bandwidthInTB', '')

    return render_template('create/os.html', **template_vars)


@app.route('/create/step-6', methods=['GET'])
@owner_required
def create_step_6():
    """Page 6: SSH Keys"""
    source = request.args
    base_state = parse_wizard_base(source)
    base_pairs = build_base_query_pairs(base_state)

    if not base_state['hostnames'] or not base_state['region']:
        flash('Please start the wizard from step 1.')
        query = build_query_string(base_pairs)
        target = url_for('create_step_1')
        if query:
            target = f"{target}?{query}"
        return redirect(target)

    if not base_state.get('os_id'):
        flash('Select an OS before continuing.')
        query = build_query_string(base_pairs)
        target = url_for('create_step_4')
        if query:
            target = f"{target}?{query}"
        return redirect(target)

    customer_id = (request.args.get('customer_id') or determine_customer_context())
    ssh_keys = load_ssh_keys(customer_id=customer_id)
    available_ssh_key_ids = {str(key['id']) for key in ssh_keys if key.get('id') is not None}
    
    selected_ssh_key_ids = [str(item) for item in base_state['ssh_key_ids']]

    back_query = build_query_string(base_pairs)
    back_url = url_for('create_step_5')
    if back_query:
        back_url = f"{back_url}?{back_query}"

    # Prepare template variables
    template_vars = {
        'ssh_keys': ssh_keys,
        'selected_ssh_key_ids': selected_ssh_key_ids,
        'base_state': base_state,
        'back_url': back_url,
    }
    
    # Add plan-specific variables
    plan_type = base_state['plan_type']
    if plan_type == 'fixed':
        template_vars['product_id'] = source.get('product_id', '')
        template_vars['extra_disk'] = source.get('extra_disk', '0')
        template_vars['extra_bandwidth'] = source.get('extra_bandwidth', '0')
    else:
        template_vars['cpu'] = source.get('cpu', '')
        template_vars['ramInGB'] = source.get('ramInGB', '')
        template_vars['diskInGB'] = source.get('diskInGB', '')
        template_vars['bandwidthInTB'] = source.get('bandwidthInTB', '')

    return render_template('create/ssh_keys.html', **template_vars)


@app.route('/create/step-7', methods=['GET', 'POST'])
@owner_required
def create_step_7():
    source = request.form if request.method == 'POST' else request.args
    base_state = parse_wizard_base(source)
    base_pairs = build_base_query_pairs(base_state)

    if not base_state['hostnames'] or not base_state['region']:
        flash('Please start the wizard from step 1.')
        query = build_query_string(base_pairs)
        target = url_for('create_step_1')
        if query:
            target = f"{target}?{query}"
        return redirect(target)

    plan_type = base_state['plan_type']
    if plan_type not in {'fixed', 'custom'}:
        flash('Choose a plan type before reviewing.')
        query = build_query_string(base_pairs)
        target = url_for('create_step_1')
        if query:
            target = f"{target}?{query}"
        return redirect(target)

    regions, region_lookup, region_configs = load_regions()
    region = region_lookup.get(base_state['region'])
    if not region:
        flash('Selected region is no longer available.')
        query = build_query_string(base_pairs)
        target = url_for('create_step_1')
        if query:
            target = f"{target}?{query}"
        return redirect(target)

    metrics = []
    plan_summary = []
    price_entries = []
    footnote = ''
    plan_state = {}
    plan_numbers = {}
    back_target = 'create_step_6'  # SSH keys is always the previous step

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
            target = url_for('create_step_3')
            if query:
                target = f"{target}?{query}"
            return redirect(target)

        product_raw = product_lookup.get(str(product_id))
        if not product_raw:
            flash('The selected product is no longer available for this region.')
            query = build_query_string(fixed_pairs)
            target = url_for('create_step_3')
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

        # Calculate RAM in MB for OS filtering (base RAM from product)
        plan_spec = product_raw.get('plan', {}).get('specification', {})
        ram_in_mb = int((plan_spec.get('ram') or plan_spec.get('ramInMB') or 1024) * 1024)
        
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
                target = url_for('create_step_3')
                if query:
                    target = f"{target}?{query}"
                return None, target
            try:
                value = int(raw_value)
            except ValueError:
                flash(f'{label} must be a whole number.')
                query = build_query_string(custom_pairs)
                target = url_for('create_step_3')
                if query:
                    target = f"{target}?{query}"
                return None, target
            if value < 1:
                flash(f'{label} must be at least 1.')
                query = build_query_string(custom_pairs)
                target = url_for('create_step_3')
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
                target = url_for('create_step_3')
                if query:
                    target = f"{target}?{query}"
                return redirect(target)
            if parsed_bandwidth < 1:
                flash('Bandwidth (TB) must be at least 1.')
                query = build_query_string(custom_pairs)
                target = url_for('create_step_3')
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
        
        # Calculate RAM in MB for OS filtering
        ram_in_mb = int(ram_value * 1024)

        back_query = build_query_string(custom_pairs)

    back_url = url_for(back_target)
    if back_query:
        back_url = f"{back_url}?{back_query}"

    # Validate OS is selected
    os_id = base_state.get('os_id', '').strip()
    if not os_id:
        flash('Select an OS before reviewing.')
        query = build_query_string(base_pairs)
        target = url_for('create_step_5')
        if query:
            target = f"{target}?{query}"
        return redirect(target)

    # Load OS details for display
    os_list = load_os_list(min_ram_in_mb=ram_in_mb, only_actives=False)
    os_lookup = {os_item.get('id'): os_item for os_item in os_list}
    selected_os_display = os_lookup.get(os_id)

    customer_id = (request.args.get('customer_id') or determine_customer_context())
    ssh_keys_catalog = load_ssh_keys(customer_id=customer_id)
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
        # Validate OS selection
        if not base_state.get('os_id'):
            flash('Please select an operating system.')
            query = build_query_string(base_pairs)
            target = url_for('create_step_5')
            if query:
                target = f"{target}?{query}"
            return redirect(target)
        
        payload = {
            'hostnames': base_state['hostnames'],
            'region': base_state['region'],
            'class': base_state.get('instance_class', 'default'),
            'assignIpv4': bool(base_state.get('assign_ipv4', False)),
            'assignIpv6': bool(base_state.get('assign_ipv6', False)),
            'osId': base_state['os_id'],
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
        if response.get('code') in ('OKAY', 'CREATED'):
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
        selected_os_display=selected_os_display,
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


@app.route('/instance/<instance_id>/delete', methods=['GET', 'POST'])
@login_required
def delete_instance(instance_id):
    redirect_response = enforce_instance_access(instance_id)
    if redirect_response:
        return redirect_response
    
    if request.method == 'GET':
        # Show confirmation page
        response = api_call('GET', f'/v1/instances/{instance_id}')
        if response.get('code') == 'OKAY':
            instance = response['data']
            return render_template('delete_instance.html', instance=instance)
        flash('Instance not found')
        return redirect(url_for('instances'))
    
    # POST: Actually delete the instance
    response = api_call('DELETE', f'/v1/instances/{instance_id}')
    if response.get('code') == 'OKAY':
        flash('Instance deleted')
    else:
        flash('Error deleting instance')
    return redirect(url_for('instances'))


@app.route('/instance/<instance_id>/poweron', methods=['GET', 'POST'])
@login_required
def poweron_instance(instance_id):
    redirect_response = enforce_instance_access(instance_id)
    if redirect_response:
        return redirect_response
    
    if request.method == 'GET':
        response = api_call('GET', f'/v1/instances/{instance_id}')
        if response.get('code') == 'OKAY':
            instance = response['data']
            return render_template('poweron_instance.html', instance=instance)
        flash('Instance not found')
        return redirect(url_for('instances'))
    
    response = api_call('POST', f'/v1/instances/{instance_id}/poweron')
    flash('Power on request sent' if response.get('code') == 'OKAY' else 'Error')
    return redirect(url_for('instance_detail', instance_id=instance_id))


@app.route('/instance/<instance_id>/poweroff', methods=['GET', 'POST'])
@login_required
def poweroff_instance(instance_id):
    redirect_response = enforce_instance_access(instance_id)
    if redirect_response:
        return redirect_response
    
    if request.method == 'GET':
        response = api_call('GET', f'/v1/instances/{instance_id}')
        if response.get('code') == 'OKAY':
            instance = response['data']
            return render_template('poweroff_instance.html', instance=instance)
        flash('Instance not found')
        return redirect(url_for('instances'))
    
    response = api_call('POST', f'/v1/instances/{instance_id}/poweroff')
    flash('Power off request sent' if response.get('code') == 'OKAY' else 'Error')
    return redirect(url_for('instance_detail', instance_id=instance_id))


@app.route('/instance/<instance_id>/reset', methods=['GET', 'POST'])
@login_required
def reset_instance(instance_id):
    redirect_response = enforce_instance_access(instance_id)
    if redirect_response:
        return redirect_response
    
    if request.method == 'GET':
        response = api_call('GET', f'/v1/instances/{instance_id}')
        if response.get('code') == 'OKAY':
            instance = response['data']
            return render_template('reset_instance.html', instance=instance)
        flash('Instance not found')
        return redirect(url_for('instances'))
    
    response = api_call('POST', f'/v1/instances/{instance_id}/reset')
    flash('Reset request sent' if response.get('code') == 'OKAY' else 'Error')
    return redirect(url_for('instance_detail', instance_id=instance_id))


@app.route('/instance/<instance_id>/change-pass', methods=['GET', 'POST'])
@login_required
def change_pass_instance(instance_id):
    redirect_response = enforce_instance_access(instance_id)
    if redirect_response:
        return redirect_response
    
    if request.method == 'GET':
        response = api_call('GET', f'/v1/instances/{instance_id}')
        if response.get('code') == 'OKAY':
            instance = response['data']
            return render_template('change_pass_instance.html', instance=instance)
        flash('Instance not found')
        return redirect(url_for('instances'))
    
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
    raw_regions = regions_response.get('data', []) if regions_response.get('code') == 'OKAY' else []
    # Only show active regions for resizing
    regions = [region for region in raw_regions if region.get('isActive')]
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
    # Handle different possible response structures
    if response.get('code') == 'OKAY':
        data = response.get('data', [])
        # Check if data is nested in another key
        if isinstance(data, dict):
            apps = data.get('applications', []) or data.get('apps', []) or []
        else:
            apps = data if isinstance(data, list) else []
    else:
        apps = []
    return render_template('applications.html', apps=apps)


if __name__ == '__main__':
    app.run(debug=True)
