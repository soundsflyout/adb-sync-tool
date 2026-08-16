# Adb-Sync-Tool

Seamlessly syncs directories between your unix machine and android.

## How it works

Similar to projects such as [adb-sync](https://github.com/google/adb-sync) and [better-adb-sync](https://github.com/jb2170/better-adb-sync), adb-sync-tool's goal is to sync between a local directory and an android device using adb. Unlike the other projects, adb-sync-tool is designed for directories that will need to be synced frequently, by employing a config file and an alias rather than ad-hoc `local` and `remote` arguments.

See it in action:
![See it in action](./ast-example.gif)

## Usage instructions

1. Clone the repository to your home directory:

```
git clone https://github.com/soundsflyout/adb-sync-tool/tree/master ~/
```

*Note: the repository must be in the home directory for the program to read the
config file properly.*

2. Enter the repository

```
cd ~/adb-sync-tool/
```

3. If rust is not installed on your system:

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

4. Install with cargo.
```
cd ~/adb-sync-tool && cargo install --path .
```

5. Create a `config.json` file in `~/adb-sync-tool/` and follow the format
   in `example_config.json`
   
6. To run the program,
- `ast push {YOUR_ALIAS}` to sync files from your local machine to the android device.
- `ast pull {YOUR_ALIAS}` to sync files from your android device to your local machine.
See `ast --help` for additional options. 
