// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

interface IFlashChain {
    // Events
    event ValidatorAdded(address indexed validator, uint256 stake);
    event ValidatorRemoved(address indexed validator);
    event StakeUpdated(address indexed validator, uint256 newStake);
    event ThresholdUpdated(uint256 newThreshold);

    // Structs
    struct ValidatorInfo {
        uint256 stake;
        uint256 joinTime;
        bool isActive;
        uint256 lastUpdateTime;
    }

    // Core functions
    function registerValidator(bytes calldata publicKey) external payable;
    function removeValidator() external;
    function updateStake() external payable;
    function isValidator(address account) external view returns (bool);
    function getValidatorInfo(address validator) external view returns (ValidatorInfo memory);
    function getActiveValidators() external view returns (address[] memory);
    function getMinimumStake() external view returns (uint256);
    function getConsensusThreshold() external view returns (uint256);
}