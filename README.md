# DFS Project - README

## Overview
The DFS (Distributed File System) project is a peer-to-peer file sharing system with two main components:
1. **DFS Engine**: The core backend system that handles file distribution and peer management.
2. **Client UI**: A web-based interface for interacting with the DFS Engine.

## Project Structure
- **dfs-engine/**: Contains the Rust-based backend implementation.
- **client-ui/**: Contains the React-based frontend implementation.

## Current Features
### DFS Engine
1. Peer-to-peer file sharing and synchronization.
2. REST API endpoints for file upload, download, and peer management.
3. Distributed Hash Table (DHT) for file location tracking.
4. TCP-based communication for peer-to-peer messaging.
5. Efficient message serialization using `bincode`.

### Client UI
1. File upload interface.
2. File listing and download functionality.
3. Peer management interface.
4. Node uptime display.

## Workflow Descriptions

### Network Node Operations (DFS Engine)
1. **Node Initialization**:
   - Creates an empty `FileSystem`.
   - Initializes an empty peer list.
   - Sets up DHT and uptime counter.
   - Starts a TCP listener for peer connections.
2. **Peer Synchronization**:
   - Requests peer file lists and exchanges missing files.
   - Updates the DHT with new file locations.

### File Operations
1. **File Upload**:
   - Splits files into chunks, hashes them, and distributes them across the network.
2. **File Download**:
   - Retrieves file chunks from peers and reassembles them.
3. **File Deletion**:
   - Removes file metadata and notifies peers.

## Next Steps

### Frontend Development (client-ui/)
1. **Implement WebSockets for Real-Time Updates**:
   - Establish a WebSocket connection with the backend.
   - Use WebSockets to receive real-time updates for file uploads, peer additions, and synchronization events.

2. **Enhance Error Management**:
   - Display user-friendly error messages for failed API calls (e.g., file not found, invalid peer address).
   - Add retry mechanisms for transient errors.
   - Handle WebSocket connection errors gracefully (e.g., reconnection attempts).

### Backend Development (dfs-engine/)
1. **Set Up WebSocket Server**:
   - Add a WebSocket server to the existing backend to enable real-time communication.
   - Broadcast events like file uploads, deletions, and peer additions to connected clients.

2. **Improve Error Management**:
   - Ensure proper error handling for REST API endpoints (e.g., invalid input, internal server errors).
   - Return descriptive error messages to the client.
   - Add logging for better debugging and monitoring.



