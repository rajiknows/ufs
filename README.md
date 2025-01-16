# DFS Engine Documentation

## Overview
The DFS (Distributed File System) Engine is a peer-to-peer file sharing system implemented in Rust. It consists of a network node system for file distribution and a REST API server for client interactions.

## System Architecture

### Core Components
1. **NetworkNode**: The main component handling peer-to-peer communications
2. **FileSystem**: Manages file storage and retrieval
3. **REST API Server**: Provides HTTP endpoints for client interactions
4. **DHT (Distributed Hash Table)**: Tracks file locations across the network

## Workflow Descriptions

### Network Node Operations

#### Node Startup Process
1. Node initialization:
   - Creates empty FileSystem
   - Initializes empty peer list
   - Sets up DHT
   - Starts uptime counter
2. Starts TCP listener for peer connections
3. Begins handling incoming connections

#### Peer Management
1. **Adding Peers**
   - New peer address is received
   - Added to known_peers list
   - Notifies existing peers about new peer
   - Initiates file sync with new peer

2. **Peer Synchronization**
   - Request peer's file list
   - Compare with local files
   - Exchange missing files
   - Update DHT with new file locations

### File Operations

#### File Upload Process
1. Client sends file path to REST API
2. Server reads file from path
3. File is chunked and hashed
4. File metadata is created and stored
5. Chunks are distributed to network
6. File hash is returned to client

#### File Download Process
1. Client requests file by hash
2. Server checks local storage for file
3. If not found locally:
   - Queries network peers
   - Downloads chunks from available peers
   - Reassembles file
4. Returns file data to client

#### File Deletion
1. Receives file hash
2. Removes file metadata
3. Removes associated chunks
4. Broadcasts deletion to peers

### REST API Endpoints

#### GET /files
1. Retrieves list of files from local node
2. Converts binary hashes to hex strings
3. Returns file list with names and hashes

#### POST /files
1. Receives file path in request body
2. Validates file existence
3. Triggers file upload process
4. Returns file hash

#### GET /files/:hash
1. Receives file hash parameter
2. Converts hex hash to binary
3. Initiates file download process
4. Returns file data if found

#### GET /peers
1. Retrieves list of known peers
2. Formats peer addresses as strings
3. Returns peer list

#### POST /peers
1. Receives peer address in request body
2. Validates address format
3. Initiates peer addition process
4. Returns success/failure status

#### GET /uptime
1. Retrieves node uptime counter
2. Returns uptime in seconds

### Message Types
The system uses various message types for peer communication:
- `GetFile`: Request file metadata
- `GetChunk`: Request specific file chunk
- `FileMetadata`: Share file information
- `ChunkData`: Transfer file chunks
- `ListFiles`: Request file listing
- `FileList`: Share file listing
- `AddPeer`: Notify about new peer
- `SyncRequest/Response`: Synchronize file lists
- `Ping/Pong`: Check peer availability

## Network Communication
1. **TCP Connections**
   - Used for all peer-to-peer communication
   - Handles large data transfers (chunks)
   - Maintains persistent connections when needed

2. **Message Serialization**
   - Uses bincode for efficient binary serialization
   - Handles all message types uniformly
   - Includes error handling for failed transfers

## State Management
1. **Shared State**
   - FileSystem state shared via Arc<Mutex>
   - Peer list managed with concurrent access
   - DHT updated across all operations

2. **Synchronization**
   - Uses Tokio for async operations
   - Mutex locks for thread-safe state access
   - Handles concurrent file operations
