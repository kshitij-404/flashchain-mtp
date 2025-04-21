const ShardRouter = artifacts.require("ShardRouter");
const ShardManager = artifacts.require("ShardManager");

async function main() {
    try {
        const shardRouter = await ShardRouter.deployed();
        const shardManager = await ShardManager.deployed();

        console.log("Setting up cross-shard routes...");

        const activeShards = await shardManager.getActiveShards();

        // Create routes between all shard pairs
        for (let i = 0; i < activeShards.length; i++) {
            for (let j = i + 1; j < activeShards.length; j++) {
                const sourceShardId = activeShards[i];
                const targetShardId = activeShards[j];

                // Set up bidirectional routes
                await shardRouter.establishRoute(
                    sourceShardId,
                    targetShardId,
                    web3.utils.toWei('1000', 'ether'), // capacity
                    15 // base latency in seconds
                );

                await shardRouter.establishRoute(
                    targetShardId,
                    sourceShardId,
                    web3.utils.toWei('1000', 'ether'),
                    15
                );

                console.log(`Routes established between shards ${sourceShardId} and ${targetShardId}`);
            }
        }

        console.log("Cross-shard routes setup completed");
    } catch (error) {
        console.error("Route setup failed:", error);
        process.exit(1);
    }
}

module.exports = main;