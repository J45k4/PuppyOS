
If making changes to ios code run this to verify your changes are correct
xcodebuild -workspace PuppyOS.xcworkspace -scheme PuppyOS -configuration Debug \
  -destination 'generic/platform=iOS' clean build