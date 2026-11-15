#!/usr/bin/env bash
# Runs the iPad app in the iOS Simulator, where the app's stderr is attached to this terminal
# and the system log is this Mac's: the device build has neither.
#
#   scripts/ios-sim.sh
#
# Builds `orb-ios` for `aarch64-apple-ios-sim`, wraps the binary in
# `dist/ios-sim/OpenRB-338.app`, boots an iPad simulator, installs and launches it, and stays
# attached until the app quits (Ctrl-C detaches). `ORB_IOS_SIM` names the simulator (a simctl
# UDID; default: the first available iPad in `xcrun simctl list devices available`).
#
# The Simulator does not enforce everything the device does (the UIScene lifecycle trap of
# iPadOS 27, for one), so the device stays the check that counts: scripts/ios-app.sh.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
target=aarch64-apple-ios-sim
app=$root/dist/ios-sim/OpenRB-338.app
bundle=com.rnsksoftronics.openrb338

sim=${ORB_IOS_SIM:-$(xcrun simctl list devices available | grep -E '^ +iPad' |
    grep -oE '[0-9A-F]{8}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{12}' | head -1)}
[ -n "$sim" ] || { echo "no iPad simulator: install a runtime in Xcode" >&2; exit 1; }

echo "ios: building orb-ios for $target"
export IPHONEOS_DEPLOYMENT_TARGET=${IPHONEOS_DEPLOYMENT_TARGET:-15.0}
cargo build --release --target "$target" -p orb-ios --manifest-path "$root/Cargo.toml"

rm -rf "$app"
mkdir -p "$app"
cp "$root/target/$target/release/orb-ios" "$app/OpenRB-338"
sed -e "s|@EXECUTABLE@|OpenRB-338|" -e "s|@BUNDLE_ID@|$bundle|" \
    -e "s|@VERSION@|$("$root/scripts/version.sh")|" -e "s|@BUILD@|1|" \
    -e "s|@MIN_OS@|$IPHONEOS_DEPLOYMENT_TARGET|" "$root/ios/Info.plist.in" > "$app/Info.plist"

xcrun simctl boot "$sim" 2>/dev/null || true
xcrun simctl bootstatus "$sim" -b >/dev/null
xcrun simctl install "$sim" "$app"
xcrun simctl terminate "$sim" "$bundle" 2>/dev/null || true
echo "ios: launching on simulator $sim"
exec xcrun simctl launch --console-pty "$sim" "$bundle"
