// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "../base/BaseShardContract.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/utils/math/SafeMath.sol";
import "@openzeppelin/contracts/utils/structs/EnumerableSet.sol";

/**
 * @title ShardRouter
 * @dev Manages cross-shard communication and routing in the FlashChain network
 */
contract ShardRouter is BaseShardContract, ReentrancyGuard {
    using SafeMath for uint256;
    using EnumerableSet for EnumerableSet.UintSet;
    using EnumerableSet for EnumerableSet.Bytes32Set;

    // Structs
    struct Route {
        uint256 sourceShardId;
        uint256 targetShardId;
        uint256 capacity;
        uint256 currentLoad;
        uint256 latency;
        uint256 successRate;
        bool isActive;
        RouteStatus status;
        mapping(bytes32 => CrossShardMessage) messages;
    }

    struct CrossShardMessage {
        bytes32 messageId;
        address sender;
        address recipient;
        uint256 timestamp;
        uint256 expiryTime;
        MessageStatus status;
        bytes payload;
        bytes32 responseMessageId;
        bytes proof;
    }

    struct RoutingMetrics {
        uint256 totalMessages;
        uint256 successfulDeliveries;
        uint256 failedDeliveries;
        uint256 averageLatency;
        uint256 congestionLevel;
        uint256 lastUpdateTime;
    }

    struct MessageBatch {
        bytes32 batchId;
        uint256 sourceShardId;
        uint256 targetShardId;
        bytes32[] messageIds;
        uint256 timestamp;
        BatchStatus status;
    }

    // Enums
    enum RouteStatus {
        Inactive,
        Active,
        Congested,
        Maintenance,
        Failed
    }

    enum MessageStatus {
        Pending,
        InTransit,
        Delivered,
        Failed,
        Expired,
        Acknowledged
    }

    enum BatchStatus {
        Pending,
        Processing,
        Completed,
        Failed
    }

    // Events
    event RouteEstablished(uint256 indexed sourceShardId, uint256 indexed targetShardId);
    event RouteStatusUpdated(uint256 indexed sourceShardId, uint256 indexed targetShardId, RouteStatus status);
    event MessageSent(bytes32 indexed messageId, uint256 indexed sourceShardId, uint256 indexed targetShardId);
    event MessageDelivered(bytes32 indexed messageId, address indexed recipient);
    event MessageFailed(bytes32 indexed messageId, string reason);
    event BatchCreated(bytes32 indexed batchId, uint256 size);
    event BatchProcessed(bytes32 indexed batchId, BatchStatus status);
    event RouteCongested(uint256 indexed sourceShardId, uint256 indexed targetShardId);
    event EmergencyRouteShutdown(uint256 indexed sourceShardId, uint256 indexed targetShardId, string reason);

    // State variables
    mapping(bytes32 => Route) public routes;
    mapping(bytes32 => MessageBatch) public messageBatches;
    mapping(uint256 => mapping(uint256 => RoutingMetrics)) public routingMetrics;
    
    EnumerableSet.UintSet private activeShards;
    EnumerableSet.Bytes32Set private activeRoutes;
    
    // Constants
    uint256 public constant MAX_BATCH_SIZE = 100;
    uint256 public constant MESSAGE_EXPIRY = 1 hours;
    uint256 public constant CONGESTION_THRESHOLD = 80; // 80%
    uint256 public constant MIN_SUCCESS_RATE = 95; // 95%
    uint256 public constant METRICS_UPDATE_INTERVAL = 5 minutes;

    // Modifiers
    modifier validRoute(uint256 sourceShardId, uint256 targetShardId) {
        bytes32 routeId = _getRouteId(sourceShardId, targetShardId);
        require(routes[routeId].isActive, "ShardRouter: Route not active");
        _;
    }

    modifier notCongested(uint256 sourceShardId, uint256 targetShardId) {
        bytes32 routeId = _getRouteId(sourceShardId, targetShardId);
        require(routes[routeId].status != RouteStatus.Congested, 
                "ShardRouter: Route congested");
        _;
    }

    /**
     * @dev Establishes a new route between shards
     * @param sourceShardId Source shard ID
     * @param targetShardId Target shard ID
     * @param capacity Maximum message capacity
     * @param latency Expected latency
     */
    function establishRoute(
        uint256 sourceShardId,
        uint256 targetShardId,
        uint256 capacity,
        uint256 latency
    ) 
        external 
        onlyRole(SHARD_ADMIN_ROLE) 
        returns (bytes32) 
    {
        require(sourceShardId != targetShardId, "ShardRouter: Invalid route");
        bytes32 routeId = _getRouteId(sourceShardId, targetShardId);
        require(!routes[routeId].isActive, "ShardRouter: Route exists");

        Route storage newRoute = routes[routeId];
        newRoute.sourceShardId = sourceShardId;
        newRoute.targetShardId = targetShardId;
        newRoute.capacity = capacity;
        newRoute.latency = latency;
        newRoute.isActive = true;
        newRoute.status = RouteStatus.Active;

        activeRoutes.add(routeId);
        emit RouteEstablished(sourceShardId, targetShardId);
        return routeId;
    }

    /**
     * @dev Sends a cross-shard message
     * @param targetShardId Target shard ID
     * @param recipient Recipient address
     * @param payload Message payload
     */
    function sendMessage(
        uint256 targetShardId,
        address recipient,
        bytes calldata payload
    ) 
        external 
        nonReentrant 
        validRoute(_getCurrentShardId(), targetShardId)
        notCongested(_getCurrentShardId(), targetShardId)
        returns (bytes32) 
    {
        bytes32 messageId = _generateMessageId();
        bytes32 routeId = _getRouteId(_getCurrentShardId(), targetShardId);
        Route storage route = routes[routeId];

        CrossShardMessage storage message = route.messages[messageId];
        message.messageId = messageId;
        message.sender = msg.sender;
        message.recipient = recipient;
        message.timestamp = block.timestamp;
        message.expiryTime = block.timestamp.add(MESSAGE_EXPIRY);
        message.status = MessageStatus.Pending;
        message.payload = payload;

        _updateRoutingMetrics(routeId);
        emit MessageSent(messageId, _getCurrentShardId(), targetShardId);
        
        return messageId;
    }

    /**
     * @dev Creates a batch of messages for efficient processing
     * @param sourceShardId Source shard ID
     * @param targetShardId Target shard ID
     * @param messageIds Array of message IDs to batch
     */
    function createMessageBatch(
        uint256 sourceShardId,
        uint256 targetShardId,
        bytes32[] calldata messageIds
    ) 
        external 
        onlyRole(VALIDATOR_ROLE) 
        returns (bytes32) 
    {
        require(messageIds.length <= MAX_BATCH_SIZE, "ShardRouter: Batch too large");
        bytes32 batchId = keccak256(abi.encodePacked(
            block.timestamp,
            sourceShardId,
            targetShardId,
            messageIds
        ));

        MessageBatch storage batch = messageBatches[batchId];
        batch.batchId = batchId;
        batch.sourceShardId = sourceShardId;
        batch.targetShardId = targetShardId;
        batch.messageIds = messageIds;
        batch.timestamp = block.timestamp;
        batch.status = BatchStatus.Pending;

        emit BatchCreated(batchId, messageIds.length);
        return batchId;
    }

    /**
     * @dev Processes a message batch
     * @param batchId Batch ID to process
     */
    function processBatch(bytes32 batchId) 
        external 
        onlyRole(VALIDATOR_ROLE) 
        nonReentrant 
    {
        MessageBatch storage batch = messageBatches[batchId];
        require(batch.status == BatchStatus.Pending, "ShardRouter: Invalid batch status");

        batch.status = BatchStatus.Processing;
        
        bool success = true;
        for (uint256 i = 0; i < batch.messageIds.length; i++) {
            if (!_processMessage(batch.messageIds[i], batch.sourceShardId, batch.targetShardId)) {
                success = false;
                break;
            }
        }

        batch.status = success ? BatchStatus.Completed : BatchStatus.Failed;
        emit BatchProcessed(batchId, batch.status);
    }

    /**
     * @dev Updates route status
     * @param sourceShardId Source shard ID
     * @param targetShardId Target shard ID
     * @param status New route status
     */
    function updateRouteStatus(
        uint256 sourceShardId,
        uint256 targetShardId,
        RouteStatus status
    ) 
        external 
        onlyRole(SHARD_ADMIN_ROLE) 
    {
        bytes32 routeId = _getRouteId(sourceShardId, targetShardId);
        require(routes[routeId].isActive, "ShardRouter: Route not active");

        routes[routeId].status = status;
        emit RouteStatusUpdated(sourceShardId, targetShardId, status);
    }

    // Internal functions

    function _getRouteId(uint256 sourceShardId, uint256 targetShardId) 
        internal 
        pure 
        returns (bytes32) 
    {
        return keccak256(abi.encodePacked(sourceShardId, targetShardId));
    }

    function _generateMessageId() internal view returns (bytes32) {
        return keccak256(abi.encodePacked(
            block.timestamp,
            msg.sender,
            tx.gasprice,
            block.number
        ));
    }

    function _processMessage(
        bytes32 messageId,
        uint256 sourceShardId,
        uint256 targetShardId
    ) 
        internal 
        returns (bool) 
    {
        bytes32 routeId = _getRouteId(sourceShardId, targetShardId);
        Route storage route = routes[routeId];
        CrossShardMessage storage message = route.messages[messageId];

        if (block.timestamp > message.expiryTime) {
            message.status = MessageStatus.Expired;
            emit MessageFailed(messageId, "Message expired");
            return false;
        }

        // Implement message processing logic
        message.status = MessageStatus.Delivered;
        emit MessageDelivered(messageId, message.recipient);
        return true;
    }

    function _updateRoutingMetrics(bytes32 routeId) internal {
        Route storage route = routes[routeId];
        RoutingMetrics storage metrics = routingMetrics[route.sourceShardId][route.targetShardId];

        metrics.totalMessages = metrics.totalMessages.add(1);
        metrics.lastUpdateTime = block.timestamp;

        if (route.currentLoad.mul(100).div(route.capacity) >= CONGESTION_THRESHOLD) {
            route.status = RouteStatus.Congested;
            emit RouteCongested(route.sourceShardId, route.targetShardId);
        }
    }

    // View functions

    function getRoute(uint256 sourceShardId, uint256 targetShardId) 
        external 
        view 
        returns (
            uint256 capacity,
            uint256 currentLoad,
            uint256 latency,
            uint256 successRate,
            bool isActive,
            RouteStatus status
        ) 
    {
        bytes32 routeId = _getRouteId(sourceShardId, targetShardId);
        Route storage route = routes[routeId];
        return (
            route.capacity,
            route.currentLoad,
            route.latency,
            route.successRate,
            route.isActive,
            route.status
        );
    }

    function getMessage(
        uint256 sourceShardId,
        uint256 targetShardId,
        bytes32 messageId
    ) 
        external 
        view 
        returns (CrossShardMessage memory) 
    {
        bytes32 routeId = _getRouteId(sourceShardId, targetShardId);
        return routes[routeId].messages[messageId];
    }

    function getMessageBatch(bytes32 batchId) 
        external 
        view 
        returns (MessageBatch memory) 
    {
        return messageBatches[batchId];
    }

    function getRoutingMetrics(uint256 sourceShardId, uint256 targetShardId) 
        external 
        view 
        returns (RoutingMetrics memory) 
    {
        return routingMetrics[sourceShardId][targetShardId];
    }

    function getActiveRoutes() external view returns (bytes32[] memory) {
        return activeRoutes.values();
    }
}