const ValidatorSet = artifacts.require("ValidatorSet");
const ShardManager = artifacts.require("ShardManager");

async function main() {
    try {
        const validatorSet = await ValidatorSet.deployed();
        const shardManager = await ShardManager.deployed();

        console.log("Setting up initial validators...");

        // Read validator addresses from environment or configuration
        const validators = process.env.INITIAL_VALIDATORS 
            ? JSON.parse(process.env.INITIAL_VALIDATORS) 
            : [];

        for (const validator of validators) {
            const stake = web3.utils.toWei('100000', 'ether');
            const publicKey = validator.publicKey;
            const metadata = JSON.stringify({
                name: validator.name,
                endpoint: validator.endpoint,
                capacity: validator.capacity
            });

            // Register validator
            await validatorSet.registerValidator(
                publicKey,
                1000, // 10% commission rate
                metadata
            );

            console.log(`Validator registered: ${validator.address}`);
        }

        // Assign validators to initial shards
        const shardsCount = await shardManager.getActiveShards().length;
        for (let shardId = 0; shardId < shardsCount; shardId++) {
            const shardValidators = validators.slice(
                shardId * 4,
                (shardId + 1) * 4
            );

            for (const validator of shardValidators) {
                await shardManager.assignValidator(shardId, validator.address);
                console.log(`Validator ${validator.address} assigned to shard ${shardId}`);
            }
        }

        console.log("Validator setup completed successfully");
    } catch (error) {
        console.error("Validator setup failed:", error);
        process.exit(1);
    }
}

module.exports = main;