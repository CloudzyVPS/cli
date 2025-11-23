from pathlib import Path
import json
import logging
import os
from functools import wraps

import requests
from flask import Flask, flash, redirect, render_template, request, session, url_for
from dotenv import load_dotenv
from werkzeug.security import check_password_hash, generate_password_hash

load_dotenv()

# Configure logging
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')

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
    return response.json()


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
    assignments = request.form.get('instances', '')
    assigned = [inst.strip() for inst in assignments.split(',') if inst.strip()]
    target['assigned_instances'] = assigned
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
    if request.method == 'POST':
        hostnames = request.form['hostnames'].split(',')
        region = request.form['region']
        product_id = request.form.get('product_id')
        instance_class = request.form['instance_class']
        cpu = request.form.get('cpu')
        ramInGB = request.form.get('ramInGB')
        diskInGB = request.form.get('diskInGB')
        bandwidthInTB = request.form.get('bandwidthInTB')
        assign_ipv4 = 'assign_ipv4' in request.form
        assign_ipv6 = 'assign_ipv6' in request.form

        data = {
            'hostnames': [h.strip() for h in hostnames],
            'region': region,
            'instance_class': instance_class,
            'assign_ipv4': assign_ipv4,
            'assign_ipv6': assign_ipv6
        }
        if product_id:
            data['product_id'] = product_id
        if cpu:
            data.setdefault('extra_resource', {})['cpu'] = int(cpu)
        if ramInGB:
            data.setdefault('extra_resource', {})['ramInGB'] = int(ramInGB)
        if diskInGB:
            data.setdefault('extra_resource', {})['diskInGB'] = int(diskInGB)
        if bandwidthInTB:
            data.setdefault('extra_resource', {})['bandwidthInTB'] = int(bandwidthInTB)

        response = api_call('POST', '/v1/instances', data=data)
        if response.get('code') == 'OKAY':
            flash('Instance created successfully')
            return redirect(url_for('instances'))
        else:
            flash(f"Error: {response.get('detail')}")
    regions_response = api_call('GET', '/v1/regions')
    regions = regions_response.get('data', []) if regions_response.get('code') == 'OKAY' else []
    return render_template('create.html', regions=regions)


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
