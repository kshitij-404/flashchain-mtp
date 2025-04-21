const ShardManager = artifacts.require("ShardManager");
const ValidatorSet = artifacts.require("ValidatorSet");
const ShardRouter = artifacts.require("ShardRouter");

async function main() {
    try {
        const shardManager = await ShardManager.deployed();
        const validatorSet = await ValidatorSet.deployed();
        const shardRouter = await ShardRouter.deployed();

        console.log("Starting maintenance tasks...");

        // Clean up inactive validators
        const validators = await validatorSet.getValidators();
        for (const validator of validators) {
            const info = await validatorSet.getValidatorInfo(validator);
            if (info.lastActiveTimestamp + 86400 < Date.now() / 1000) {
                await validatorSet.removeValidator(validator);
                console.log(`Removed inactive validator: ${validator}`);
            }
        }

        // Optimize shard load
        const activeShards = await shardManager.getActiveShards();
        for (const shardId of activeShards) {
            const info = await shardManager.getShardInfo(shardId);
            if (info.currentLoad > info.capacity * 0.9) {
                await shardManager.optimizeShard(shardId);
                console.log(`Optimized overloaded shard: ${shardId}`);
            }
        }

        // Clean up expired messages
        const routes = await shardRouter.getActiveRoutes();
        for (const routeId of routes) {
            await shardRouter.cleanupExpiredMessages(routeId);
            console.log(`Cleaned up expired messages for route: ${routeId}`);
        }

        console.log("Maintenance tasks completed");
    } catch (error) {
        console.error("Maintenance failed:", error);
        process.exit(1);
    }
}

module.exports = main;