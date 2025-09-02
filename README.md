Thalassocracy — Dev Quickstart

Run the server (local dev):
- Windows PowerShell: `setx RUST_LOG "info,bevy_renet=debug,renet=debug" & cargo run -p server`
- Bash: `RUST_LOG=info,bevy_renet=debug,renet=debug cargo run -p server`

Run the client (local dev):
- Windows PowerShell: `setx RUST_LOG "info,bevy_renet=debug,renet=debug" & cargo run -p client -- --connect-timeout-secs 5`
- Bash: `RUST_LOG=info,bevy_renet=debug,renet=debug cargo run -p client -- --connect-timeout-secs 5`

Server configuration:
- Default config path: `server/config.toml`
- Keys:
  - `port`: UDP listen port (default `61234`)
  - `max_clients`: maximum simultaneous clients
  - `tick_hz`: simulation tick rate
  - `snapshot_hz`: target snapshot send rate
  - `public_addr` (optional): address advertised in netcode tokens.
    - For local dev, omit this (defaults to `127.0.0.1:<port>` if bound to `0.0.0.0`).
    - For remote hosting, set to your public IP/hostname and port, e.g. `"203.0.113.10:61234"`.

Windows firewall (server):
- Allow inbound UDP on the server port:
  - `netsh advfirewall firewall add rule name="thalasso-udp" dir=in action=allow protocol=UDP localport=61234`

Client options:
- `--server <ip:port>`: override server address (default `127.0.0.1:61234`)
- `--headless`: run without window/rendering
- `--name <display_name>`: optional display name
- `--connect-timeout-secs <n>`: timeout before exiting (default `5`)

Notes:
- Client and server use a shared netcode protocol id and real wall-clock time for stable handshakes.
- For remote use, ensure `public_addr` is set and firewall/NAT forwards UDP.
