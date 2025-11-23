from pathlib import Path
import json
import logging
import os
import sys
from functools import wraps

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


@app.context_processor
def inject_user():
    return {'current_user': get_current_user()}


@app.route('/login', methods=['GET', 'POST'])
def login():
    if 'username' in session:
        return redirect(url_for('instances'))
    if request.method == 'POST':
        username = normalize_username(request.form['username'])
        password = request.form['password']
        user = users.get(username)
        if user and check_password_hash(user['password'], password):
            session['username'] = username
            flash('Welcome back!')
            return redirect(url_for('instances'))
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


@app.route('/')
def root():
    if 'username' in session:
        return redirect(url_for('instances'))
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
def create():
    regions_response = api_call('GET', '/v1/regions')
    regions = regions_response.get('data', []) if regions_response.get('code') == 'OKAY' else []

    plan_type = 'fixed'
    form_data = {}
    selected_region = None
    selected_product_id = ''
    errors: list[str] = []

    if request.method == 'POST':
        form_data = request.form.to_dict()
        form_data['assign_ipv4'] = 'on' if 'assign_ipv4' in request.form else ''
        form_data['assign_ipv6'] = 'on' if 'assign_ipv6' in request.form else ''

        plan_type = form_data.get('plan_type', 'fixed')
        if plan_type not in {'fixed', 'custom'}:
            plan_type = 'fixed'

        selected_region = form_data.get('region') or (regions[0]['id'] if regions else None)
        selected_product_id = form_data.get('product_id', '')

        if plan_type == 'custom':
            selected_product_id = ''

        hostnames_raw = form_data.get('hostnames', '')
        hostnames = [h.strip() for h in hostnames_raw.split(',') if h.strip()]
        if not hostnames:
            errors.append('Provide at least one hostname (comma-separated).')

        if not selected_region:
            errors.append('Select a deployment region.')

        assign_ipv4 = 'assign_ipv4' in request.form
        assign_ipv6 = 'assign_ipv6' in request.form

        instance_class = form_data.get('instance_class', 'default')

        data = {
            'hostnames': hostnames,
            'region': selected_region,
            'class': instance_class,
            'assignIpv4': assign_ipv4,
            'assignIpv6': assign_ipv6,
        }

        if plan_type == 'fixed':
            if not selected_product_id:
                errors.append('Select a product when using the fixed plan option.')
            else:
                data['productId'] = selected_product_id

        def parse_int_field(field_name: str, label: str, *, min_value: int | None = None, max_value: int | None = None):
            raw_value = form_data.get(field_name)
            if raw_value is None or raw_value == '':
                return None
            try:
                value = int(raw_value)
            except ValueError:
                errors.append(f'{label} must be a number.')
                return None
            if min_value is not None and value < min_value:
                errors.append(f'{label} must be at least {min_value}.')
                return None
            if max_value is not None and value > max_value:
                errors.append(f'{label} must be at most {max_value}.')
                return None
            return value

        extras = {}
        if plan_type == 'fixed':
            disk_value = parse_int_field('fixed_diskInGB', 'Extra disk (GB)', min_value=1)
            bandwidth_value = parse_int_field('fixed_bandwidthInTB', 'Extra bandwidth (TB)', min_value=1)
            if disk_value is not None:
                extras['diskInGB'] = disk_value
            if bandwidth_value is not None:
                extras['bandwidthInTB'] = bandwidth_value
        elif plan_type == 'custom':
            cpu_value = parse_int_field('cpu', 'CPU', min_value=1)
            ram_value = parse_int_field('ramInGB', 'RAM (GB)', min_value=1)
            disk_value = parse_int_field('diskInGB', 'Disk (GB)', min_value=1)
            bandwidth_value = parse_int_field('bandwidthInTB', 'Bandwidth (TB)', min_value=1)

            if cpu_value is None:
                errors.append('CPU is required for custom plans.')
            else:
                extras['cpu'] = cpu_value
            if ram_value is None:
                errors.append('RAM (GB) is required for custom plans.')
            else:
                extras['ramInGB'] = ram_value
            if disk_value is None:
                errors.append('Disk (GB) is required for custom plans.')
            else:
                extras['diskInGB'] = disk_value
            if bandwidth_value is not None:
                extras['bandwidthInTB'] = bandwidth_value

            config = next((region.get('config', {}) for region in regions if region.get('id') == selected_region), {})
            ram_threshold = config.get('ramThresholdInGB')
            disk_threshold = config.get('diskThresholdInGB')
            if ram_threshold and extras.get('ramInGB') is not None and extras['ramInGB'] < ram_threshold:
                errors.append(f'RAM must be at least {ram_threshold} GB for the selected region.')
            if disk_threshold and extras.get('diskInGB') is not None and extras['diskInGB'] < disk_threshold:
                errors.append(f'Disk must be at least {disk_threshold} GB for the selected region.')
        else:
            errors.append('Unknown plan type submitted.')

        if extras:
            data['extraResource'] = extras

        floating_ip_count = parse_int_field('floating_ip_count', 'Floating IP count', min_value=0, max_value=5)
        if floating_ip_count is not None:
            data['floatingIPCount'] = floating_ip_count

        ssh_raw = form_data.get('ssh_key_ids', '')
        if ssh_raw.strip():
            ssh_ids = []
            for chunk in ssh_raw.split(','):
                token = chunk.strip()
                if not token:
                    continue
                if not token.isdigit():
                    errors.append('SSH key IDs must be comma-separated numbers.')
                    ssh_ids = []
                    break
                ssh_ids.append(int(token))
            if ssh_ids:
                data['sshKeyIds'] = ssh_ids

        if errors:
            for message in errors:
                flash(message)
        else:
            response = api_call('POST', '/v1/instances', data=data)
            if response.get('code') == 'OKAY':
                flash('Instance created successfully')
                return redirect(url_for('instances'))
            flash(f"Error: {response.get('detail')}")

    else:
        plan_type = request.args.get('plan_type', 'fixed')
        if plan_type not in {'fixed', 'custom'}:
            plan_type = 'fixed'
        selected_region = request.args.get('region') or (regions[0]['id'] if regions else None)
        selected_product_id = request.args.get('product_id', '')
        form_data = {
            'hostnames': '',
            'instance_class': 'default',
            'assign_ipv4': 'on',
            'assign_ipv6': '',
            'fixed_diskInGB': '',
            'fixed_bandwidthInTB': '',
            'cpu': '',
            'ramInGB': '',
            'diskInGB': '',
            'bandwidthInTB': '',
            'floating_ip_count': '',
            'ssh_key_ids': '',
        }

    selected_region = selected_region or (regions[0]['id'] if regions else None)
    form_data.setdefault('instance_class', 'default')
    form_data.setdefault('assign_ipv4', 'on')
    form_data.setdefault('assign_ipv6', '')

    products = []
    if selected_region:
        products_response = api_call('GET', '/v1/products', params={'regionId': selected_region})
        if products_response.get('code') == 'OKAY':
            products = products_response.get('data', [])
        else:
            flash('Unable to load products for the selected region.')

    region_configs = {region.get('id'): region.get('config', {}) for region in regions}
    region_lookup = {region.get('id'): region for region in regions}

    return render_template(
        'create.html',
        regions=regions,
        products=products,
        selected_region=selected_region,
        selected_product_id=selected_product_id,
        plan_type=plan_type,
        form_data=form_data,
        region_configs=region_configs,
        region_lookup=region_lookup,
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


@app.route('/products', methods=['GET', 'POST'])
@owner_required
def products():
    if request.method == 'POST':
        region_id = request.form['region_id']
        response = api_call('GET', '/v1/products', params={'regionId': region_id})
        products = response.get('data', []) if response.get('code') == 'OKAY' else []
        regions_response = api_call('GET', '/v1/regions')
        regions = regions_response.get('data', []) if regions_response.get('code') == 'OKAY' else []
        return render_template('products.html', products=products, regions=regions, selected_region=region_id)
    regions_response = api_call('GET', '/v1/regions')
    regions = regions_response.get('data', []) if regions_response.get('code') == 'OKAY' else []
    return render_template('products.html', products=[], regions=regions, selected_region=None)


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
