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
2) Run the command from this directory, `.\create-dev-box.ps1 -VMRoot D:\dev -VMType overlaybd -VMName mirrortestvm`
3) After the script completes wait a minute or two for the VM to initialize
4) Open Hyper-V manager and a new VM should be running
5) Double click on the VM, when prompted for username enter `chief` (TODO: Temporary common username)
6) Choose a new password and login
7) Double check cloud-init is finished, 
    1. `sudo su`, must be root to check the logs
    2. `tail -f /var/log/cloud-init-output.log`, the last message should say finished and if it's not complete tail should be following it to completion
8) Reboot the machine, `reboot` -- This is so that you can see the ip address from the Hyper-V Manager. It will be under the networking tab.

### Connecting to the VM 

You can either connect through the Hyper-V manager, or through your own terminal. If you connect through the Hyper-v manager you will be missing some quality of life features, but it's fast and easy. 

If you want to connect through terminal there are some steps you need to complete before that is possible. 

1) First you need to add your public ssh-key to the VM, currently this might be a bit of a pain and is something that can be automated by the script but that's required
2) Next open up a powershell prompt on the host. It's important that you do not try to ssh from a WSL machine, because you won't be able to resolve the IP. 
3) Find the ip address in the Hyper-V manager
4) ssh to the machine, likely will need to specify <username>@<ip> -- This experience will be improved eventually.

