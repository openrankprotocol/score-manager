// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Script, console} from "forge-std/Script.sol";
import {ComputeManager} from "../src/ComputeManager.sol";

contract ComputeManagerScript is Script {

    function run() public {
        // Load environment variables (such as private key, etc.)
        uint256 deployerPrivateKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        address submitter = vm.envAddress("SUBMITTER");
        address computer = vm.envAddress("COMPUTER");
        address verifier = vm.envAddress("VERIFIER");
    
        // Start broadcasting the transaction
        vm.startBroadcast(deployerPrivateKey);

        address[] memory submitters = new address[](1);
        submitters[0] = submitter;

        address[] memory computers = new address[](1);
        computers[0] = computer;

        address[] memory verifiers = new address[](2);
        verifiers[0] = verifier;

        ComputeManager computeManager = new ComputeManager(submitters, computers, verifiers);

        // Print the contract address
        console.log("ComputeManager contract deployed at:", address(computeManager));

        vm.stopBroadcast();
    }
}
