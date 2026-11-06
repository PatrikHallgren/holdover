# Milestone 10 checklist

Version 1.5.0, desktop package revision 2. `versionCode` is 1005001, above the 1.5.0-1 install's 1005000. The usual formula for 1.5.1 is also 1005001, so a later build has to use a code above 1005001. No tag and no release are created here.

This checklist is for a person with the desktop and the phone on the same network. The automated tests do not do these steps. They run two libraries on one machine. A physical desktop-to-phone sync has not been done, and a real `.holdover` round-trip on the phone has not been done either.

The phone install has to be signed with the same key as the 1.4.0 sideload (`versionCode` 1004001). That key stays on the omawork computer. CI signs with `packaging/android/ci-test.keystore` only. A CI APK will not update the phone.

## Pair

1. On the desktop, Sync, Phone, Show pairing code. The main area shows a black-on-white picture of the code, the time it expires, and the text of the code under the picture. The text starts with `holdover://pair?v=1`. Scan the picture. Paste the text if the camera cannot read the picture.
2. On the phone, Sync, Phone, paste that line into Pairing code and press Pair with this code. Or choose a photograph of the code.
3. On the desktop, the phone's name and key fingerprint appear. Accept this device.
4. Both sides list the other under Paired devices. Cancel clears the code. A second use of the same code is refused. An old code, after 10 minutes, is refused.

## Sync, with the network discovery working

1. On one device, Ready for a sync. Leave that screen open.
2. On the other, Find on this network. The paired device should appear. Press Start sync on that row.
3. The preview lists records that would change. Nothing is written yet.
4. Apply sync. Open a record that was edited on only one side and confirm it arrived. Edit a different field on each side, sync again, and confirm both edits are present. Edit the same field on both sides and confirm the preview asks you to choose. Archive a record on one side and confirm the other side archives it after Apply.
5. Sync again with nothing new. The preview says nothing changes.

## Sync, with discovery blocked

1. Ready for a sync on one device. Note the address and port, from the pairing code or from the port on that screen.
2. On the other device, leave Find on this network unused, or confirm it reports that discovery did not answer.
3. Type the address and port. Start sync. The preview and Apply behave as above. An address for a device that was not paired is refused and writes nothing.

## Offline and failure

1. Turn off Wi-Fi on one device, or unplug the network, and start a sync. It fails. The library is unchanged.
2. Start a sync, then Cancel, or leave the preview and press Cancel, before Apply. The library is unchanged.
3. A `.holdover` export and import still works, including when the two devices are not on a network. Rifle serials stay off that file. Precise coordinates stay off unless the box is checked.

## What this pass did not do

- Install 1.5.0 revision 2 (`versionCode` 1005001) on the OnePlus 13. The 1.5.0-1 desktop build showed the pairing text in the sidebar and no picture.
- Pair the phone with the Omarchy desktop, or finish a sync. That has not passed.
- Photograph the desktop picture with the OnePlus camera. An automated test decodes a JPEG made from the same picture.
- Confirm mDNS on that home network.
- Confirm a `.holdover` file copied onto the phone and back.
