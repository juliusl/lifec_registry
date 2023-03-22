# Hyper-V Dev Box Tool

## Use Case

In most cases developing on WSL2 is good enough, however WSL2's kernel does not include all the kernel features available upstream. In the case of overlaybd, the driver requires TCMU. That means the only way 
to test this locally is to either a) create a vm on azure, or b) create a vm in hyper-v. This folder is about how to do the latter, however the user-data config (./configs/overlaybd-dev.yml) could later be applied
to an Azure VM as well.

## Requirements

- Virtualization needs to be enabled in bios
- Hyper-V Manager needs to be enabled
- Script is cross compatible between powershell 5 and 7, however it might be a good idea to install powershell 7

## Instructions 

1) Start an admin powershell prompt
2) Run the command from this directory, `.\create-dev-box.ps1 -VMRoot D:\dev -VMType overlaybd -VMName mirrortestvm -EnableSSH $true`
3) Wait for the VM to reboot (A VM will not have an IP Address listed in Hyper-V Manager before being rebooted)
4) Open a PS terminal, and ssh w/ `ssh yourusername@ipaddress` this should prompt you for your ssh-key password
