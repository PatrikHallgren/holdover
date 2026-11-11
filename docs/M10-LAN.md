# Milestone 10 checklist

Version 1.5.4, desktop package revision 1. `versionCode` is 1005004. Pairing was confirmed on 1.5.2 (`versionCode` 1005002, desktop package 1.5.2-1). 1.5.3 (`versionCode` 1005003) was installed on both devices and the sync still failed: Ready bound a new random port because the previous listener still held 43720, and the desktop firewall dropped Wi-Fi. 1.5.0-1 was 1005000 and 1.5.0 revision 2 was 1005001. The usual formula for 1.5.1 is also 1005001, which is already installed, so that number was skipped. No tag and no release are created here.

This checklist is for a person with the desktop and the phone on the same network. The automated tests do not do these steps. They run two libraries on one machine. Pairing on the OnePlus 13 and the Omarchy desktop passed on 1.5.2. A physical sync has not passed, and a real `.holdover` round-trip on the phone has not been done either.

The phone install has to be signed with the same key as the 1.4.0 sideload (`versionCode` 1004001). That key stays on the omawork computer. CI signs with `packaging/android/ci-test.keystore` only. A CI APK will not update the phone.

## Pair

1. On the desktop, Sync, Phone, Show pairing code. The main area shows a black-on-white picture of the code, the time it expires, and the text of the code under the picture. The text starts with `holdover://pair?v=1`. Scan the picture. Paste the text if the camera cannot read the picture.
2. On the phone, Sync, Phone, paste that line into Pairing code and press Pair with this code. Or press Choose photograph or Take photograph. The phone says it is reaching the other Holdover, then that the other device has the request. A photograph that cannot be read, an empty choice, or a computer that cannot be reached stays on screen as an error. Nothing is paired by the photograph itself.
3. On the desktop, the phone's name and key fingerprint appear. Accept this device. Until Accept is pressed, the phone keeps saying the request is waiting.
4. Both sides list the other under Paired devices. Cancel clears the code. A second use of the same code is refused. An old code, after 10 minutes, is refused.

## Sync, with the network discovery working

1. On one device, Ready for a sync. The phone panel names this device and the port it is listening on. The usual port is 43720. Leave that screen open.
2. On the other, Find on this network. The paired device should appear. Press Start sync on that row. The screen says it is reaching the paired Holdover.
3. The preview lists records that would change. Nothing is written yet.
4. Apply sync. Open a record that was edited on only one side and confirm it arrived. Edit a different field on each side, sync again, and confirm both edits are present. Edit the same field on both sides and confirm the preview asks you to choose. Archive a record on one side and confirm the other side archives it after Apply.
5. Sync again with nothing new. The preview says nothing changes.

## Sync, with discovery blocked

1. Ready for a sync on one device. Read the port on the line that starts with "This device". It stays the same across restarts unless that port is already taken.
2. On the other device, Start sync on the paired row. Leave the address and port fields blank. The sync tries each saved address, the LAN address before a Tailscale address, at the saved port and at 43720. A saved port of 0 is not dialed.
3. Find on this network can stay empty when an always-on VPN has the multicast. The phone still says that no other Holdover answered. Start sync does not depend on that search. A failure stays on screen and lists each address and port. When a Wi-Fi address times out, it also says the firewall may be dropping it. On Linux, Wi-Fi needs `sudo ufw allow 43720/tcp` and `sudo ufw allow 5353/udp` when the default policy is to drop incoming traffic. Tailscale can already be allowed while Wi-Fi is not. A log line `lan action=sync-begin` is written with those tries. On the phone that line is logcat tag `Holdover`. On the desktop it is `journalctl -t holdover`.
4. The preview and Apply behave as above. An address for a device that was not paired is refused and writes nothing.

## Offline and failure

1. Turn off Wi-Fi on one device, or unplug the network, and start a sync. It fails. The library is unchanged.
2. Start a sync, then Cancel, or leave the preview and press Cancel, before Apply. The library is unchanged.
3. A `.holdover` export and import still works, including when the two devices are not on a network. Rifle serials stay off that file. Precise coordinates stay off unless the box is checked.

## What this pass did not do

- Install 1.5.4 (`versionCode` 1005004) on the OnePlus 13 and on the Omarchy desktop. 1.5.3 is installed on both. Pairing on 1.5.2 passed. The 1.5.3 sync did not: the desktop listener moved from 43720 to a new port on each Ready, the dial used port 0, and ufw dropped the Wi-Fi SYNs.
- Finish a sync between the phone and the Omarchy desktop. That has not passed.
- Confirm mDNS while Tailscale is always on. Find on this network can stay empty in that case. The saved addresses and port 43720 are the path that does not need discovery.
- Confirm a `.holdover` file copied onto the phone and back.
