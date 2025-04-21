const NetworkParams = artifacts.require("NetworkParams");
const ShardManager = artifacts.require("ShardManager");
const ValidatorSet = artifacts.require("ValidatorSet");

const { BN } = require('web3-utils');
const fs = require('fs');

async function main() {
    try {
        const networkParams = await NetworkParams.deployed();
        const shardManager = await ShardManager.deployed();
        const validatorSet = await ValidatorSet.deployed();

        console.log("Initializing FlashChain network...");

        // Initial network configuration
        const networkConfig = {
            maxShards: 32,
            minNodesPerShard: 4,
            maxNodesPerShard: 100,
            consensusThreshold: 67, // 67%
            blockInterval: new BN('15'), // 15 seconds
            epochDuration: new BN('3600'), // 1 hour
            validatorMinStake: web3.utils.toWei('100000', 'ether'),
            delegatorMinStake: web3.utils.toWei('1000', 'ether'),
            crossShardTxGasMultiplier: 2,
            baseRewardRate: 500, // 5% annual in basis points
            slashingPenalty: 1000, // 10% in basis points
            dynamicSharding: true
        };

        await networkParams.updateNetworkConfig(networkConfig);
        console.log("Network parameters initialized");

        // Write configuration to file
        fs.writeFileSync(
            './config/network-config.json',
            JSON.stringify(networkConfig, null, 2)
        );

        console.log("Network initialization completed successfully");
    } catch (error) {
        console.error("Network initialization failed:", error);
        process.exit(1);
    }
}

module.exports = main;