# ComputeManager Smart Contract

## Deployment Instructions

1. **Navigate to the Project Directory**
   ```sh
   cd contracts
   ```

2. **Build the Smart Contract**
   ```sh
   forge build
   ```

3. **Configure Environment**
   - Create a `.env` file by copying `.env.example` and add your private key.  
     This key is used for submitting the deployment transaction to the blockchain.
     If you deploy contract on `anvil` network, you should use the private key from `anvil` test wallet.
     If you deploy contract on other testnet(e.g. Amoy Polygon testnet), you should use the private key of wallet you use for development.

4. **Update Addresses**
   - Modify the addresses of `submitters`, `computers`, and `verifiers` in `DeployComputeManager.s.sol` if needed.
     Especially, you should check addresses depending on which network you want to deploy contract.
     If you deploy contract on `anvil` network, you should use the local openrank `computer` & `verifier` address.
     Plus, you should use the `anvil` test wallet address as `submitter` one.
     If you deploy contract on other testnet(e.g. Amoy Polygon tesnet), you should use openrank devnet `computer` & `verifier` address.
     Also, you should use the wallet address which is used as smart contract client tx submission, as `submitter` one.

5. **Simulate Deployment**
   ```sh
   forge script script/DeployComputeManager.s.sol
   ```
   - This simulation provides deployment guarantees and gas cost estimates.  
   - **Note**: Ensure the `.env` file is present.

6. **Deploy to Testnet/Localnet**
   - To deploy on a different network (e.g., Ethereum mainnet), adjust the `rpc-url` accordingly.
   ```sh
   forge script script/DeployComputeManager.s.sol --rpc-url https://rpc-amoy.polygon.technology/ --broadcast --optimize --optimizer-runs 4000
   ```

   To deploy on anvil local network, use this one.
   ```sh
   forge script script/DeployComputeManager.s.sol --rpc-url http://localhost:8545 --broadcast --optimize --optimizer-runs 4000
   ```

   **NOTE**: You should double-check the `submitters`, `computers` and `verifiers` addresses.

7. **Update Configuration**
   - Copy the deployed contract address into `client/config.toml` for use in the smart contract client.
   - **Note**: If you updated contract source code, you should copy the contract abi json to `client/abi` dir, and rebuild `client`.  
   Otherwise, the `client` keeps using old abi json.  
   ```sh
   cp out/ComputeManager.sol/ComputeManager.json ../client/abi  
   ```

