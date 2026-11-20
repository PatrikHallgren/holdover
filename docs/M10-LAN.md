# Milestone 10 checklist

Version 1.5.5, desktop package revision 1. `versionCode` is 1005005. Pairing was confirmed on 1.5.2 (`versionCode` 1005002, desktop package 1.5.2-1). 1.5.3 and 1.5.4 were installed. 1.5.4 listens on 43720, and the phone's last sync dialed `100.x:43720` and got `refused` when the desktop was not sitting on Ready. 1.5.5 keeps that listener up while the desktop app is open and dials only the Tailscale address. 1.5.0-1 was 1005000 and 1.5.0 revision 2 was 1005001. The usual formula for 1.5.1 is also 1005001, which is already installed, so that number was skipped. No tag and no release are created here.

This checklist is for a person with Tailscale running on the desktop and the phone. The automated tests do not do these steps. They run two libraries on one machine. Pairing on the OnePlus 13 and the Omarchy desktop passed on 1.5.2. A physical sync has not passed, and a real `.holdover` round-trip on the phone has not been done either. Wi-Fi discovery is not part of this path. The desktop firewall already allows the Tailscale interface, so no new firewall rule is required.

The phone install has to be signed with the same key as the 1.4.0 sideload (`versionCode` 1004001). That key stays on the omawork computer. CI signs with `packaging/android/ci-test.keystore` only. A CI APK will not update the phone.

## Pair

1. On the desktop, with Tailscale connected, Sync, Phone, Show pairing code. The main area shows a black-on-white picture of the code, the time it expires, and the text of the code under the picture. The text starts with `holdover://pair?v=1` and includes the desktop's 100.x address and port 43720. If Tailscale has no 100.x address, the screen says so and does not show a code. Scan the picture. Paste the text if the camera cannot read the picture.
2. On the phone, Sync, Phone, paste that line into Pairing code and press Pair with this code. Or press Choose photograph or Take photograph. The phone says it is reaching the other Holdover, then that the other device has the request. A photograph that cannot be read, an empty choice, or a computer that cannot be reached stays on screen as an error. Nothing is paired by the photograph itself.
3. On the desktop, the phone's name and key fingerprint appear. Accept this device. Until Accept is pressed, the phone keeps saying the request is waiting.
4. Both sides list the other under Paired devices. Cancel clears the code. A second use of the same code is refused. An old code, after 10 minutes, is refused.

## Sync

1. Leave the desktop app open. Sync, Phone, should say `Sync ready over Tailscale at <name/100.x>:43720`. There is no Ready button. If Tailscale has no 100.x address, that line is an error instead.
2. On the phone, Sync, Phone. Each paired desktop has Sync now. Leave the Tailscale address blank when the pair already has a 100.x address. If it does not, type the desktop's Tailscale IP or MagicDNS name. Press Sync now. The screen says it is reaching that Holdover.
3. The preview lists records that would change. Nothing is written yet.
4. Apply sync. Open a record that was edited on only one side and confirm it arrived. Edit a different field on each side, sync again, and confirm both edits are present. Edit the same field on both sides and confirm the preview asks you to choose. Archive a record on one side and confirm the other side archives it after Apply.
5. Sync again with nothing new. The preview says nothing changes.
6. A failure stays on screen and names the address, port 43720, and the reason (refused, timeout, no tailscale, or auth failed). On the phone that line is `adb logcat -s Holdover`. On the desktop it is `journalctl -t holdover`. A device that was not paired is refused and writes nothing.

## Offline and failure

1. Turn off Wi-Fi on one device, or unplug the network, and start a sync. It fails. The library is unchanged.
2. Start a sync, then Cancel, or leave the preview and press Cancel, before Apply. The library is unchanged.
3. A `.holdover` export and import still works, including when the two devices are not on a network. Rifle serials stay off that file. Precise coordinates stay off unless the box is checked.

## What this pass did not do

- Install 1.5.5 (`versionCode` 1005005) on the OnePlus 13 and on the Omarchy desktop. 1.5.4 is installed on both. Pairing on 1.5.2 passed. The 1.5.4 listener stayed on 43720, and the phone dial of that Tailscale port was refused when the desktop was not on Ready.
- Finish a sync between the phone and the Omarchy desktop. That has not passed.
- Confirm a `.holdover` file copied onto the phone and back.
