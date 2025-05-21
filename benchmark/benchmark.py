import json
import random
from locust import HttpUser, task, between

class TransactionAPIUser(HttpUser):
    """
    A Locust user class that simulates users hitting a transaction API
    with transaction hashes loaded from a JSON file.
    """
    # Simulate user waiting time between requests (in seconds)
    # This will be a random float between 1 and 2 seconds.
    wait_time = between(1, 2)

    # Base URL for the API endpoint
    # IMPORTANT: Ensure your local API is running on this address and port.
    host = "http://localhost:8081"

    def on_start(self):
        """
        This method is called when a Locust user starts.
        It loads the transaction hashes from the 'tx_hashes.json' file.
        """
        self.tx_hashes = []
        try:
            with open('tx_hashes.json', 'r') as f:
                self.tx_hashes = json.load(f)
            print(f"Loaded {len(self.tx_hashes)} transaction hashes from tx_hashes.json")
            if not self.tx_hashes:
                print("Warning: tx_hashes.json is empty. No transactions to test.")
        except FileNotFoundError:
            print("Error: tx_hashes.json not found. Please run the Monero Transaction Hash Collector script first.")
            # Exit or handle gracefully if the file is essential
            self.environment.runner.quit() # This will stop the Locust test if the file is missing
        except json.JSONDecodeError:
            print("Error: Could not decode JSON from tx_hashes.json. Ensure it's a valid JSON array.")
            self.environment.runner.quit() # This will stop the Locust test

    @task
    def get_transaction_by_hash(self):
        """
        This task simulates a user requesting a transaction by its hash.
        It picks a random hash from the loaded list and makes a GET request.
        """
        if not self.tx_hashes:
            print("No transaction hashes available to test.")
            return

        # Choose a random transaction hash from the loaded list
        random_tx_hash = random.choice(self.tx_hashes)

        # Define the API endpoint path with the chosen hash
        endpoint = f"/api/transaction/{random_tx_hash}"

        # Make the GET request
        # The 'name' parameter groups requests in Locust's statistics.
        # Using a parameterized name like "/api/transaction/[hash]" is good practice
        # to avoid creating a unique entry for every single hash in the statistics.
        self.client.get(endpoint, name="/api/transaction/[tx_hash]")
