import requests
import json
import time

def collect_tx_hashes(start_block: int = 3000000, target_count: int = 100, output_filename: str = "tx_hashes.json"):
    """
    Collects transaction hashes from the xmr.se API, starting from a given block number,
    until a target number of unique transaction hashes are collected.
    The collected hashes are then stored in a JSON file.

    Args:
        start_block (int): The starting block number to fetch data from.
        target_count (int): The desired number of transaction hashes to collect.
        output_filename (str): The name of the JSON file to store the hashes.
    """
    base_url = "https://xmr.se/api/block/"
    current_block_number = start_block
    tx_hashes = []
    collected_count = 0

    print(f"Starting to collect {target_count} transaction hashes from block {start_block}...")

    while collected_count < target_count:
        url = f"{base_url}{current_block_number}"
        print(f"Fetching data from: {url}")

        try:
            response = requests.get(url, timeout=10) # Set a timeout for the request
            response.raise_for_status()  # Raise an HTTPError for bad responses (4xx or 5xx)
            data = response.json()

            # Navigate to the transaction list within the response
            # The structure is response.data.txs[]
            if 'data' in data and 'txs' in data['data'] and isinstance(data['data']['txs'], list):
                transactions = data['data']['txs']
                for tx in transactions:
                    if not tx['coinbase']:
                        tx_hashes.append(tx['tx_hash'])
                        collected_count += 1
                        if collected_count >= target_count:
                            break # Exit inner loop if target count is reached

                print(f"Collected {collected_count}/{target_count} transaction hashes.")
            else:
                print(f"Warning: 'data' or 'txs' key not found or 'txs' is not a list in response for block {current_block_number}.")

        except requests.exceptions.HTTPError as http_err:
            print(f"HTTP error occurred for block {current_block_number}: {http_err}")
            if response.status_code == 404:
                print(f"Block {current_block_number} not found. Skipping to next block.")
            # Consider adding a break or more robust error handling if 404s are frequent
        except requests.exceptions.ConnectionError as conn_err:
            print(f"Connection error occurred: {conn_err}. Retrying in 5 seconds...")
            time.sleep(5) # Wait before retrying on connection error
            continue # Skip incrementing block number and retry current block
        except requests.exceptions.Timeout as timeout_err:
            print(f"Request timed out: {timeout_err}. Retrying in 5 seconds...")
            time.sleep(5) # Wait before retrying on timeout
            continue # Skip incrementing block number and retry current block
        except json.JSONDecodeError as json_err:
            print(f"JSON decode error for block {current_block_number}: {json_err}. Response content: {response.text[:200]}...")
        except Exception as err:
            print(f"An unexpected error occurred for block {current_block_number}: {err}")

        current_block_number += 1
        time.sleep(0.1)  # Small delay to avoid overwhelming the API

    # Ensure we only store the exact target_count if more were collected in the last iteration
    final_tx_hashes = tx_hashes[:target_count]

    try:
        with open(output_filename, 'w') as f:
            json.dump(final_tx_hashes, f, indent=4)
        print(f"\nSuccessfully collected {len(final_tx_hashes)} transaction hashes and saved to '{output_filename}'")
    except IOError as io_err:
        print(f"Error writing to file '{output_filename}': {io_err}")

if __name__ == "__main__":
    # You can modify the starting block and target count here
    collect_tx_hashes(start_block=3000000, target_count=100)