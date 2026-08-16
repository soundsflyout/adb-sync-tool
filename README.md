# Adb-Sync-Tool

Seamlessly syncs directories between your unix machine and android.

## How it works

Similar to projects such as [adb-sync](https://github.com/google/adb-sync) and [better-adb-sync](https://github.com/jb2170/better-adb-sync), adb-sync-tool's goal is to sync between a local directory and an android device using adb. Unlike the other projects, adb-sync-tool is designed for directories that will need to be synced frequently, by employing a config file and an alias rather than ad-hoc `local` and `remote` arguments.

![See it in action](https://github.com/soundsflyout/adb-sync-tool/ast-example.gif)
