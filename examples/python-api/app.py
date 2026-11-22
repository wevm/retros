"""A small JSON API with `/health`, `/users`, and `/users/<id>`.

`python3 app.py 0` listens on a free port and prints the port it chose.
"""

import json
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

USERS = [{"id": id, "name": f"User {id}"} for id in range(1, 101)]


class Handler(BaseHTTPRequestHandler):
    """Answers each route with JSON, and anything else with 404."""

    def do_GET(self):
        id = self.path.removeprefix("/users/")

        if self.path == "/health":
            body = {"ok": True}
        elif self.path == "/users":
            body = USERS
        elif id.isdigit() and 1 <= int(id) <= len(USERS):
            body = USERS[int(id) - 1]
        else:
            self.send_error(404)
            return

        data = json.dumps(body).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *args):
        pass


if __name__ == "__main__":
    server = ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), Handler)
    print(server.server_address[1], flush=True)
    server.serve_forever()
