#!/bin/bash

# Create virtual environment if it doesn't exist
if [ ! -d "venv" ]; then
    echo "Creating virtual environment..."
    python3 -m venv venv
fi

# Activate virtual environment
echo "Activating virtual environment..."
source venv/bin/activate

# Install requirements if needed
if [ ! -f "venv/installed" ]; then
    echo "Installing requirements..."
    pip install -r requirements.txt
    touch venv/installed
fi

# Run the application
echo "Starting Zyffiliate..."
python app.py