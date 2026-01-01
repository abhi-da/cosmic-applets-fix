# cosmic-applets-fix
Contains the fixes of some cosmic applets. Too lazy to add more description.


```markdown
# Cosmic Audio Applet (Stable Shell Fix)

A modified version of the official COSMIC Audio Applet designed for maximum stability. 

Instead of interfacing directly with the PipeWire library (which can cause panics and random output switching on some hardware), this version acts as a "Stateless Controller." It sends high-level shell commands to `wpctl` and `playerctl`, ensuring that your audio devices never switch automatically unless *you* tell them to.

## 🚀 Key Features

* **Crash Proof:** Uses decoupled shell logic. Dragging the volume slider will **never** cause your output to randomly switch to HDMI or Headphones.
* **Modern Media UI:** A completely redesigned media player section featuring:
    * Large, full-width Album Art.
    * Vertical layout: Art → Controls → Title → Artist.
* **Device Switching:** Working dropdowns to switch between Speakers and Headphones.
* **Microphone Control:** Full support for input volume and muting.


## 🛠️ Prerequisites

This applet relies on standard system utilities to control audio and media. Ensure they are installed:

```bash
sudo apt update
sudo apt install wireplumber playerctl

```

* **wireplumber (`wpctl`):** Used for volume and device control.
* **playerctl:** Used for Play/Pause/Next media controls.

## 📦 Installation

### 1. Build from Source

Ensure you have Rust and Cargo installed, then run:

```bash
cargo build --release

```

### 2. Install the Binary

Move the compiled binary to your system's binary directory. We rename it to avoid conflicts with the official applet.
Move the fixed executable to /usr/bin/

```bash
sudo cp target/release/cosmic-applet-audio /usr/bin/usr-audio-applet

```
or 
```bash
sudo /home/location/cosmic-applet-audio-fix /usr/bin/usr-audio-applet
```

### 3. Register the Applet

Create a `.desktop` file so the COSMIC Panel can recognize it.

```bash
sudo bash -c 'cat <<EOF > /usr/share/applications/com.usr.AudioApplet.desktop
[Desktop Entry]
Name=Sound-Fix
Comment=Stable volume control with Album Art
Exec=usr-audio-applet
Icon=audio-volume-high-symbolic
Type=Application
Terminal=false
Categories=COSMIC;
StartupNotify=true
NoDisplay=true
X-CosmicApplet=true
X-CosmicShrinkable=true
X-CosmicHoverPopup=End
X-OverflowPriority=10
EOF'

```

### 4. Activate

Restart the panel to reload the applet list:

```bash
killall cosmic-panel

```

1. Go to *Cosmic Settings* > *Desktop* > *Panel* > *Configure panel applets*.
2. Remove the original **Sound** applet.
3. Add **Sound-Fix** from the list.



## 🤝 Credits

* Based on the original [Cosmic Applets](https://github.com/pop-os/cosmic-applets) by System76.
* Modified to use `std::process::Command` for stateless interaction.

```

```
