from flask import Flask, render_template, request, redirect, url_for, flash
import requests
import os
import logging
from dotenv import load_dotenv

load_dotenv()

# Configure logging
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')

app = Flask(__name__)
app.secret_key = 'your_secret_key'  # Change this

API_BASE_URL = os.getenv('API_BASE_URL')
API_TOKEN = os.getenv('API_TOKEN')

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
    return response.json()

@app.route('/')
def index():
    response = api_call('GET', '/v1/instances')
    if response.get('code') == 'OKAY':
        instances = response['data']['instances']
    else:
        instances = []
        flash('Error fetching instances')
    return render_template('index.html', instances=instances)

@app.route('/create', methods=['GET', 'POST'])
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
            return redirect(url_for('index'))
        else:
            flash(f'Error: {response.get("detail")}')
    # For GET, fetch regions
    regions_response = api_call('GET', '/v1/regions')
    regions = regions_response.get('data', []) if regions_response.get('code') == 'OKAY' else []
    return render_template('create.html', regions=regions)

@app.route('/instance/<instance_id>')
def instance_detail(instance_id):
    response = api_call('GET', f'/v1/instances/{instance_id}')
    if response.get('code') == 'OKAY':
        instance = response['data']
        return render_template('instance_detail.html', instance=instance)
    else:
        flash('Instance not found')
        return redirect(url_for('index'))

@app.route('/instance/<instance_id>/delete', methods=['POST'])
def delete_instance(instance_id):
    response = api_call('DELETE', f'/v1/instances/{instance_id}')
    if response.get('code') == 'OKAY':
        flash('Instance deleted')
    else:
        flash('Error deleting instance')
    return redirect(url_for('index'))

@app.route('/instance/<instance_id>/poweron', methods=['POST'])
def poweron_instance(instance_id):
    response = api_call('POST', f'/v1/instances/{instance_id}/poweron')
    flash('Power on request sent' if response.get('code') == 'OKAY' else 'Error')
    return redirect(url_for('instance_detail', instance_id=instance_id))

@app.route('/instance/<instance_id>/poweroff', methods=['POST'])
def poweroff_instance(instance_id):
    response = api_call('POST', f'/v1/instances/{instance_id}/poweroff')
    flash('Power off request sent' if response.get('code') == 'OKAY' else 'Error')
    return redirect(url_for('instance_detail', instance_id=instance_id))

@app.route('/instance/<instance_id>/reset', methods=['POST'])
def reset_instance(instance_id):
    response = api_call('POST', f'/v1/instances/{instance_id}/reset')
    flash('Reset request sent' if response.get('code') == 'OKAY' else 'Error')
    return redirect(url_for('instance_detail', instance_id=instance_id))

@app.route('/instance/<instance_id>/change-pass', methods=['POST'])
def change_pass_instance(instance_id):
    response = api_call('POST', f'/v1/instances/{instance_id}/change-pass')
    if response.get('code') == 'OKAY':
        password = response['data']['password']
        flash(f'Password changed: {password}')
    else:
        flash('Error changing password')
    return redirect(url_for('instance_detail', instance_id=instance_id))

@app.route('/instance/<instance_id>/resize', methods=['GET', 'POST'])
def resize_instance(instance_id):
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
                if cpu: data['extraResource']['cpu'] = int(cpu)
                if ramInGB: data['extraResource']['ramInGB'] = int(ramInGB)
                if diskInGB: data['extraResource']['diskInGB'] = int(diskInGB)
                if bandwidthInTB: data['extraResource']['bandwidthInTB'] = int(bandwidthInTB)

        response = api_call('POST', f'/v1/instances/{instance_id}/resize', data=data)
        if response.get('code') == 'OKAY':
            flash('Resize request sent')
            return redirect(url_for('instance_detail', instance_id=instance_id))
        else:
            flash(f'Error: {response.get("detail")}')
    # GET: show form
    instance_response = api_call('GET', f'/v1/instances/{instance_id}')
    instance = instance_response.get('data') if instance_response.get('code') == 'OKAY' else None
    regions_response = api_call('GET', '/v1/regions')
    regions = regions_response.get('data', []) if regions_response.get('code') == 'OKAY' else []
    return render_template('resize.html', instance=instance, regions=regions)

@app.route('/instance/<instance_id>/change-os', methods=['GET', 'POST'])
def change_os_instance(instance_id):
    if request.method == 'POST':
        os_id = request.form['os_id']
        response = api_call('POST', f'/v1/instances/{instance_id}/change-os', data={'osId': os_id})
        if response.get('code') == 'OKAY':
            flash('OS change request sent')
            return redirect(url_for('instance_detail', instance_id=instance_id))
        else:
            flash(f'Error: {response.get("detail")}')
    # GET: show form
    os_response = api_call('GET', '/v1/os')
    os_list = os_response.get('data', {}).get('os', []) if os_response.get('code') == 'OKAY' else []
    return render_template('change_os.html', instance_id=instance_id, os_list=os_list)

@app.route('/instance/<instance_id>/subscription-refund')
def subscription_refund(instance_id):
    response = api_call('GET', f'/v1/instances/{instance_id}/subscription-refund')
    if response.get('code') == 'OKAY':
        refund = response['data']
        return render_template('subscription_refund.html', refund=refund, instance_id=instance_id)
    else:
        flash('Error fetching refund details')
        return redirect(url_for('instance_detail', instance_id=instance_id))

@app.route('/bulk-subscription-refund', methods=['GET', 'POST'])
def bulk_subscription_refund():
    if request.method == 'POST':
        ids = request.form['ids'].split(',')
        data = {'ids': [id_.strip() for id_ in ids]}
        response = api_call('POST', '/v1/instances/bulk-subscription-refund', data=data)
        if response.get('code') == 'OKAY':
            refunds = response['data']
            return render_template('bulk_refund_result.html', refunds=refunds)
        else:
            flash(f'Error: {response.get("detail")}')
    return render_template('bulk_refund.html')

@app.route('/regions')
def regions():
    response = api_call('GET', '/v1/regions')
    regions = response.get('data', []) if response.get('code') == 'OKAY' else []
    return render_template('regions.html', regions=regions)

@app.route('/products', methods=['GET', 'POST'])
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
def os_list():
    response = api_call('GET', '/v1/os')
    os_list = response.get('data', {}).get('os', []) if response.get('code') == 'OKAY' else []
    return render_template('os.html', os_list=os_list)

@app.route('/applications')
def applications():
    response = api_call('GET', '/v1/applications')
    apps = response.get('data', []) if response.get('code') == 'OKAY' else []
    return render_template('applications.html', apps=apps)

if __name__ == '__main__':
    app.run(debug=True)