// Group together functions for cli outputs for easy testing.

pub mod cli_tools {
    use adb_client::{ADBDeviceExt, server::ADBServer};
    use dialoguer::MultiSelect;
    use std::error::Error;

    pub fn get_devices(server: &mut ADBServer) -> Result<Vec<String>, Box<dyn Error>> {
        let connected_devices: Vec<String> = server
            .devices()?
            .iter()
            .map(|x| x.identifier.clone())
            .collect();
        Ok(connected_devices)
    }

    pub fn get_storage_info(server: &mut ADBServer) -> Result<(), Box<dyn Error>> {
        let connected_devices: Vec<String> = server
            .devices()?
            .iter()
            .map(|x| x.identifier.clone())
            .collect();
        println!("{:?}", connected_devices);
        let selection = MultiSelect::new()
            .with_prompt("Choose device(s): \n Use up/down or k/j to move up/down and select with Space. Press Enter to confirm.")
            .items(&connected_devices)
            .interact()?;

        for idx in selection {
            let mut stdout = Vec::new();
            let mut device = server.get_device_by_name(&connected_devices[idx])?;
            device.shell_command(&"df -h", Some(&mut stdout), None)?;
            let stdout_str: String = String::from_utf8(stdout)?;
            println!("{}", stdout_str);
        }
        Ok(())
    }
}
