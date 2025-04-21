const NetworkParams = artifacts.require("NetworkParams");
const ShardManager = artifacts.require("ShardManager");
const ShardRouter = artifacts.require("ShardRouter");

async function main() {
    try {
        const networkParams = await NetworkParams.deployed();
        const shardManager = await ShardManager.deployed();
        const shardRouter = await ShardRouter.deployed();

        console.log("Starting network monitoring...");

        // Monitor network metrics
        setInterval(async () => {
            const metrics = await networkParams.getNetworkMetrics();
            console.log("\nNetwork Metrics:");
            console.log("Total Nodes:", metrics.totalNodes.toString());
            console.log("Active Shards:", metrics.activeShards.toString());
            console.log("Total Transactions:", metrics.totalTransactions.toString());
            console.log("Average Block Time:", metrics.averageBlockTime.toString());
            console.log("Network Load:", metrics.networkLoad.toString());
        }, 60000); // Every minute

        // Monitor shard status
        setInterval(async () => {
            const activeShards = await shardManager.getActiveShards();
            console.log("\nShard Status:");
            for (const shardId of activeShards) {
                const info = await shardManager.getShardInfo(shardId);
                console.log(`Shard ${shardId}:`);
                console.log("  Load:", info.currentLoad.toString());
                console.log("  Status:", info.status.toString());
            }
        }, 30000); // Every 30 seconds

        // Monitor cross-shard routes
        setInterval(async () => {
            const activeRoutes = await shardRouter.getActiveRoutes();
            console.log("\nRoute Status:");
            for (const routeId of activeRoutes) {
                const route = await shardRouter.getRoute(routeId);
                console.log(`Route ${routeId}:`);
                console.log("  Load:", route.currentLoad.toString());
                console.log("  Success Rate:", route.successRate.toString());
            }
        }, 45000); // Every 45 seconds

        console.log("Network monitoring started");
    } catch (error) {
        console.error("Monitoring failed:", error);
        process.exit(1);
    }
}

module.exports = main;