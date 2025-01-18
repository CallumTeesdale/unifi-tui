# unifi-tui

[![Crates.io](https://img.shields.io/crates/v/unifi-tui)](https://crates.io/crates/unifi-tui)
[![License](https://img.shields.io/crates/l/unifi-tui)](LICENSE)


A terminal user interface (TUI) using the [unifi-rs](https://crates.io/crates/unifi-rs) library for the UniFi Network API.

Currently, a work in progress. Intend to add more features as the unifi-rs library gets more features. 

Can view sites, devices, clients.

## Usage
```shell
unifi-tui --url {url} --api-key {api-key} --insecure
```

Or with environment variables
```shell
export UNIFI_URL={url}
export UNIFI_API_KEY={api-key}

unifi-tui
```

## Screenshots
![Clients](./doc/clients.png)
![Devices](./doc/devices.png)