<# 
    .SYNOPSIS 
    Provisions a VM and configures using a cloud-init user_data

    .PARAMETER VMRoot
    Root directory for storing VM's and the image cache

    .PARAMETER VMType
    VM Type name which will be used as the directory name w/ specific cloud-init config

    .PARAMETER VMName
    VM Name used on VM creation, will also be used as the name of the .vhdx
    
    .PARAMETER EnableSSH
    Enable the current user's ssh-pub-key as an authorized key on the VM

    .PARAMETER UserDataSource
    Path to source .yml for cloud-config user-data, see https://cloudinit.readthedocs.io/en/latest/reference/examples.html for examples
#>
param (
    [Parameter(Mandatory)]
    [string]$VMRoot,
    [Parameter(Mandatory)]
    [string]$VMType,
    [Parameter(Mandatory)]
    [string]$VMName,
    [Parameter(Mandatory = $false)]
    [bool]$EnableSSH,
    [Parameter(Mandatory = $false)]
    [string]$UserDataSource = "./configs/containerd-dev.yml"
)

# Get the ID and security principal of the current user account
$myWindowsID = [System.Security.Principal.WindowsIdentity]::GetCurrent()
$myWindowsPrincipal = new-object System.Security.Principal.WindowsPrincipal($myWindowsID)
 
# Get the security principal for the Administrator role
$adminRole = [System.Security.Principal.WindowsBuiltInRole]::Administrator
 
# Check to see if we are currently running "as Administrator"
if (!$myWindowsPrincipal.IsInRole($adminRole)) {
    Write-Error "Must run script as administrator"
    exit 1
}

# ADK Download - `winget install Microsoft.WindowsADK`
# You only need to install the deployment tools
$oscdimgPath = ".\bin\windows-adk\oscdimg.exe"

# Download qemu-img from here: http://www.cloudbase.it/qemu-img-windows/
$qemuImgPath = ".\bin\qemu\qemu-img.exe"

# Update this to the release of Ubuntu that you want
$ubuntuPath = "https://cloud-images.ubuntu.com/focal/current/focal-server-cloudimg-amd64"

$virtualSwitchName = "Default Switch"
$imageCachePath = "$($VMRoot)\imagecache"
$vmPath = "$($VMRoot)\$($VMType)"
$vmConfigPath = "$($vmPath)\$($VMName)\config"
$vhdx = "$($vmPath)\$($VMName).vhdx"
$metaDataIso = "$($vmConfigPath)\metadata.iso"
$nocloudPath = "$($vmConfigPath)\NoCloud"

# Get the timestamp of the latest build on the Ubuntu cloud-images site
$ubuntuManifestURI = "$ubuntuPath.manifest"
$manifestResponse = (Invoke-WebRequest $ubuntuManifestURI)
$lastModified = $manifestResponse.Headers.'Last-Modified'

if ($PSVersionTable.PSVersion.Major -gt 5) {
    $lastModified = [DateTime]$lastModified[0]
}
else {
    $lastModified = [DateTime]$lastModified
}

$stamp = $lastModified.ToFileTimeUtc()

$metadata = @"
instance-id: guid-$([GUID]::NewGuid())
local-hostname: $($VMName)
"@

$networkconfig = @"
version: 2
ethernets:
  eth0:
    dhcp4: true
"@

# Check Paths
if (!(test-path $imageCachePath)) { mkdir $imageCachePath }

if (!(test-path $vmConfigPath)) {
    mkdir -p $vmConfigPath
}

if (!(test-path $nocloudPath)) {
    mkdir -p $nocloudPath
}

if (!(test-path $vmPath)) {
    mkdir $vmPath
}

# Helper function for no error file cleanup
Function cleanupFile ([string]$file) { if (test-path $file) { Remove-Item $file } }

# Create new virtual machine and start it
# Delete the VM if it is around
If ((Get-VM | Where-Object name -eq $VMName).Count -gt 0)
{ Stop-VM $VMName -TurnOff -Confirm:$false -Passthru | Remove-VM -Force }
cleanupFile $vhdx
cleanupFile $metaDataIso

# Make temp location
if (!(test-path "$($imageCachePath)\ubuntu-$($stamp).img")) {
    # If we do not have a matching image - delete the old ones and download the new one
    Remove-Item "$($imageCachePath)\ubuntu-*.img"
    Invoke-WebRequest "$($ubuntuPath).img" -UseBasicParsing -OutFile "$($imageCachePath)\ubuntu-$($stamp).img"
}

# Convert cloud image to VHDX
& $qemuImgPath convert -f qcow2 "$($imageCachePath)\ubuntu-$($stamp).img" -O vhdx -o subformat=dynamic $vhdx

if ($EnableSSH) {
    $sshKey = Get-Content "c:\Users\$($env:USERNAME)\.ssh\id_rsa.pub"

    $vendorData = @"
## template: jinja
#cloud-config
merge_how:
 - name: list
   settings: [append]
 - name: dict
   settings: [no_replace, recurse_list]

packages_update: true
packages_upgrade: true
power_state:
  mode: reboot

users:
 - default
 - name: $($env:USERNAME)
   sudo: ALL=(ALL) NOPASSWD:ALL
   shell: /bin/bash
   ssh_authorized_keys:
    - $($sshKey)
"@

    if ($PSVersionTable.PSVersion.Major -gt 5) {
        # Output meta, network, and user data to files
        Set-Content "$($nocloudPath)\vendor-data" ([byte[]][char[]] "$vendorData") -AsByteStream
    }
    else {
        # Output meta, network, and user data to files
        Set-Content "$($nocloudPath)\vendor-data" ([byte[]][char[]] "$vendorData") -Encoding Byte
    }
}

if ($PSVersionTable.PSVersion.Major -gt 5) {
    # Output meta, network, and user data to files
    Set-Content "$($nocloudPath)\meta-data" ([byte[]][char[]] "$metadata") -AsByteStream
    Set-Content "$($nocloudPath)\network-config" ([byte[]][char[]] "$networkconfig") -AsByteStream
}
else {
    # Output meta, network, and user data to files
    Set-Content "$($nocloudPath)\meta-data" ([byte[]][char[]] "$metadata") -Encoding Byte
    Set-Content "$($nocloudPath)\network-config" ([byte[]][char[]] "$networkconfig") -Encoding Byte
}

if (!(test-path "$($nocloudPath)\user-data")) {
    Set-Content "$($nocloudPath)\user-data" (Get-Content $UserDataSource)
}

# Create meta data ISO image
& $oscdimgPath $nocloudPath $metaDataIso -j2 -lcidata

Resize-VHD -Path $vhdx -SizeBytes 512GB

New-VM $VMName -MemoryStartupBytes 4096mb -BootDevice VHD -VHDPath $vhdx -Generation 2 `
    -SwitchName $virtualSwitchName -Path $vmPath | Out-Null
Set-VM -Name $VMName -ProcessorCount 2 -AutomaticStopAction ShutDown -AutomaticStartAction StartIfRunning -AutomaticStartDelay (Get-Random -Minimum 100 -Maximum 800)
Set-VMFirmware -VMName $VMName -EnableSecureBoot Off -FirstBootDevice (Get-VMHardDiskDrive -VMName $VMName)
Get-VM -VMname $VMName | Enable-VMIntegrationService -Name *
Add-VMDvdDrive -VMName $VMName
Set-VMDvdDrive -VMName $VMName -Path $metaDataIso
Start-VM $VMName

Write-Information "The VM will now start. Wait for it to reboot and then you can ssh to it."
