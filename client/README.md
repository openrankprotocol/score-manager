# ComputeManager smart contract client

## How to run

1. **Navigate to Directory**
    ```sh
    cd client
    ```

2. **Prerequisites**
    - Create the `config.toml` by copying the example file(e.g., `config.toml.local`)
    - Follow the `contracts/README.md`, to deploy smart contract and copy address to `config.toml` file

3. **Configure Environment**
    - Create a `.env` file by copying the `.env.example` 
    - Add the `SUBMITTER_SECRET_KEY`  
      The `SUBMITTER_SECRET_KEY` is for submitting tx from smart contract client, to blockchain.  
      This key corresponds to `SUBMITTER` address in contract deployment.  
    - **NOTE**: Current `.env.example` contains the key of `anvil` 1st test wallet

4. **Run the client**
    - start-interval-submit
    ```sh
    cargo r -- start-interval-submit
    ```

    - post tx on chain with id
    ```sh
    cargo r -- post-tx-on-chain [TX_PREFIX]:[TX_HASH]
    ```
    Ex:
    ```sh
    cargo r -- post-tx-on-chain ComputeAssignment:9def96d5947326324c72633df4ed65acf79fe34861d38071072cbc4fea4015d9
