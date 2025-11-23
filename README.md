# Zyffiliate

A cloud provider service built using the Cloudzy Developer API. This is a server-side rendered web application (no JavaScript) for managing VPS instances.

## Features

- **Instances Management**:
  - List all instances
  - Create new instances (fixed plans or custom configurations)
  - View detailed instance information
  - Resize instances (fixed or custom)
  - Change operating system
  - Power on/off/reset instances
  - Change password
  - Delete instances
  - View subscription refunds
- **Bulk Operations**:
  - Bulk subscription refunds
- **Resources**:
  - List regions
  - List products by region
  - List operating systems
  - List applications
- **Design**: Professional, production-ready stylesheet with Digital Ocean color palette (blues, grays), smooth transitions, responsive grid system, and accessibility features.

## Setup

1. Install dependencies:
   ```bash
   pip install -r requirements.txt
   ```

2. Configure your API credentials in `.env`:
   ```
   API_BASE_URL=https://api.cloudzy.com/developers
   API_TOKEN=your_api_token_here
   ```

3. Run the application:
   ```bash
   ./run.sh
   ```
   Or manually:
   ```bash
   python app.py
   ```

4. Open http://localhost:5000 in your browser.

## Requirements

- Python 3.8+
- Flask
- Requests
- Python-dotenv