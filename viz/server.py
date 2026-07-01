import os
import time
import threading
from http.server import SimpleHTTPRequestHandler
from socketserver import ThreadingMixIn, TCPServer
from queue import Queue

PIPE_PATH = '/tmp/bezel-osd'
clients = []

def tail_pipe():
    print(f"Ensuring pipe exists at {PIPE_PATH}...")
    if not os.path.exists(PIPE_PATH):
        try:
            os.mkfifo(PIPE_PATH)
        except OSError as e:
            print(f"Failed to create fifo: {e}")

    print("Listening for Bezel OSD messages...")
    while True:
        try:
            # Opening fifo for reading. This will block until a writer opens it.
            with open(PIPE_PATH, 'r') as fifo:
                while True:
                    line = fifo.readline()
                    if not line:
                        break # EOF, writer closed
                    msg = line.strip()
                    if msg:
                        print(f"Received gesture: {msg}")
                        # Send to all connected SSE clients
                        for q in list(clients):
                            q.put(msg)
        except Exception as e:
            print("Error reading pipe:", e)
            time.sleep(1)

class SSEHandler(SimpleHTTPRequestHandler):
    def do_GET(self):
        if self.path == '/stream':
            self.send_response(200)
            self.send_header('Content-type', 'text/event-stream')
            self.send_header('Cache-Control', 'no-cache')
            self.send_header('Access-Control-Allow-Origin', '*')
            self.send_header('Connection', 'keep-alive')
            self.end_headers()
            
            q = Queue()
            clients.append(q)
            print(f"Client connected. Total clients: {len(clients)}")
            try:
                # Send an initial connection event
                self.wfile.write(b"data: connected\n\n")
                self.wfile.flush()
                
                while True:
                    msg = q.get()
                    self.wfile.write(f"data: {msg}\n\n".encode('utf-8'))
                    self.wfile.flush()
            except Exception as e:
                # Client disconnected
                pass
            finally:
                if q in clients:
                    clients.remove(q)
                print(f"Client disconnected. Total clients: {len(clients)}")
        else:
            super().do_GET()

class ThreadedTCPServer(ThreadingMixIn, TCPServer):
    allow_reuse_address = True
    daemon_threads = True

if __name__ == '__main__':
    # Start pipe reader thread
    t = threading.Thread(target=tail_pipe, daemon=True)
    t.start()
    
    # Start web server
    PORT = 8080
    server = ThreadedTCPServer(('0.0.0.0', PORT), SSEHandler)
    print(f"Serving visualization on http://localhost:{PORT}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nShutting down...")
        server.server_close()
