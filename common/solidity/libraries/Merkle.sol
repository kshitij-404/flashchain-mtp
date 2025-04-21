// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

library Merkle {
    function verifyProof(
        bytes32[] memory proof,
        bytes32 root,
        bytes32 leaf
    ) internal pure returns (bool) {
        bytes32 computedHash = leaf;

        for (uint256 i = 0; i < proof.length; i++) {
            bytes32 proofElement = proof[i];

            if (computedHash < proofElement) {
                computedHash = keccak256(abi.encodePacked(computedHash, proofElement));
            } else {
                computedHash = keccak256(abi.encodePacked(proofElement, computedHash));
            }
        }

        return computedHash == root;
    }

    function getRoot(bytes32[] memory leaves) internal pure returns (bytes32) {
        require(leaves.length > 0, "Merkle: empty leaves");

        bytes32[] memory currentLevel = leaves;
        
        while (currentLevel.length > 1) {
            bytes32[] memory nextLevel = new bytes32[]((currentLevel.length + 1) / 2);
            
            for (uint256 i = 0; i < nextLevel.length; i++) {
                uint256 leftIndex = i * 2;
                uint256 rightIndex = leftIndex + 1;
                
                bytes32 left = currentLevel[leftIndex];
                bytes32 right = rightIndex < currentLevel.length 
                    ? currentLevel[rightIndex] 
                    : left;
                
                nextLevel[i] = keccak256(abi.encodePacked(left, right));
            }
            
            currentLevel = nextLevel;
        }
        
        return currentLevel[0];
    }
}