## 1. Tracker

In this implementation, one tracker only responsible for tracking information of one file.

A tracker maintain a list of peers participating in the distribution of the particular file.

There are 3 events a tracker needs to handle:

1. **Peer joining**

   A client can requests to participate in the distribution list of peers at any time.

   To join a swarm and become a peer, it simply needs to send a request to tracker, and the tracker will response a list of all active peers distributing that file.

   The request is known as **Join Request**.

   (no rejection mechanism for simplicity)

   

2. **Peer leaving**

   A tracker will assign and track an expired time for every peer in the peer list, and the tracker side  will regularly check whether the current global time exceeds the expired time of a peer, if it does, it will simply discard the peer in the peer list.

   Hence, each peer in the swarm needs to constantly send an existent request to tracker to proof he is still active, and the tracker will update the expired time of that peer.

   The request is known as **Active Proof Request**.

   (no rejection mechanism for simplicity)

   

3. **Request of peer list**

   If a client wants to request the list of peers, the tracker simply return a list of all peers to that client unreservedly. 

   The request is known as **Peer List Request**.



Thus, a tracker altogether maintaining:

1. List of active peers
2. Expired times for each peer.



## 2. Peer (Client)

Recall that in our implementation, one tracker only responsible for one file. When a peer wants to download a file, he will somehow know the IP address of the tracker responsible for the file.

A peer maintains a list of other peers participating in the file distribution called **neighbors**. 

The file is separated into bytes chunks called **chunk**, the maximum size of chunk is **maximum chunk size**. A peer maintaining a list of chunks currently having, called **downloaded chunks**.

Thus a peer maintaining:

1. **neighbors** - known peers in the swarm.
2. **downloaded chunks**





### 2.1 Client and Tracker

This section describes events between client (peer) and tracker.

1. **Joining a swarm**

   If a peer wants to participate in a swarm, it send a **Join Request** to the tracker, once the tracker acknowledged, it is count as a member of the swarm.

   Once a peer joined a swarm, he needs to constantly send a **Active Proof Request** to inform he is still active.

   

2. **Getting a peer list**

   A client sends a **Peer List Request** to tracker to get an active list of peers.

   

3. **Leaving a swarm**

   A peer can just cut of the connection and leave, the tracker will automatically discard him from the peer list after it is expired.



### 2.2 Peer and Neighbors

Initially, a peer gets its **neighbors** list from tracker. **Downloaded chunks** are empty.

#### 2.2.1 Requesting Neighbors

This section discusses the request initialized by a peer.

1. **Obtaining list of downloaded chunks from neighbors**

   To know which peer to request chunk from, a peer periodically send a query to all its **neighbors**, and each neighbor will reply their **downloaded chunks** to query initializer correspondingly. 

   The request is known as **Chunks Query Request**

   

2. **Getting a chunk**

   Knowing which neighbor has the desired chunk to download, a peer can request to download from that neighbor. After finishing downloads a chunk, he appends the new chunk to his **downloaded chunks**.

   The request is known as **Fetch Chunk Request**.



#### 2.2.2 Responding Neighbors

This section discusses how a peer responds an incoming request from its **neighbors**.

1. Incoming **Chunks Query Request**

   The peer simply returns his **downloaded chunks** to the requesting neighbor.

   (no rejection mechanism for simplicity)

   

2. Incoming **Get Chunk Request**

   The peer simply return the chunk to the requesting neighbor.

   (no rejection mechanism for simplicity)
