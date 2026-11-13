"""Seeded symmetric RTT/loss injection for real game clients (stdlib only)."""
import heapq
import random
import select
import socket
import threading
import time


class UdpProxy:
    def __init__(self, server, rtt_ms=0, loss_percent=0, seed=1):
        self.client = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.client.bind(('127.0.0.1', 0))
        self.server = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.server.connect(server)
        self.address = self.client.getsockname()
        self.delay = rtt_ms / 2000
        self.loss = loss_percent / 100
        self.rng = random.Random(seed)
        self.stopped = threading.Event()
        self.counts = dict(up=0, down=0, up_dropped=0, down_dropped=0)
        self.error = None
        self.thread = threading.Thread(target=self.run, daemon=True)
        self.thread.start()

    def run(self):
        peer, serial, queue = None, 0, []
        try:
            while not self.stopped.is_set():
                readable, _, _ = select.select([self.client, self.server], [], [], 0.001)
                for sock in readable:
                    if sock is self.client:
                        packet, peer = self.client.recvfrom(65535)
                        direction = 'up'
                    else:
                        packet = self.server.recv(65535)
                        direction = 'down'
                    self.counts[direction] += 1
                    if self.rng.random() < self.loss:
                        self.counts[direction+'_dropped'] += 1
                    else:
                        serial += 1
                        heapq.heappush(queue, (time.monotonic()+self.delay, serial, direction, packet))
                now = time.monotonic()
                while queue and queue[0][0] <= now:
                    _, _, direction, packet = heapq.heappop(queue)
                    if direction == 'up':
                        self.server.send(packet)
                    elif peer is not None:
                        self.client.sendto(packet, peer)
        except Exception as error:
            self.error = repr(error)

    def close(self):
        self.stopped.set()
        self.thread.join(timeout=2)
        self.client.close()
        self.server.close()
