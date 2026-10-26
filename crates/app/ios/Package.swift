// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "MediaManagerAds",
    platforms: [.iOS(.v15)],
    products: [.library(name: "MediaManagerAds", type: .dynamic, targets: ["MediaManagerAds"])],
    dependencies: [
        .package(url: "https://github.com/googleads/swift-package-manager-google-mobile-ads.git", exact: "13.11.0"),
        .package(url: "https://github.com/googleads/swift-package-manager-google-user-messaging-platform.git", exact: "3.1.0")
    ],
    targets: [
        .target(name: "MediaManagerAds", dependencies: [
            .product(name: "GoogleMobileAds", package: "swift-package-manager-google-mobile-ads"),
            .product(name: "GoogleUserMessagingPlatform", package: "swift-package-manager-google-user-messaging-platform")
        ], linkerSettings: [.unsafeFlags(["-ObjC"])])
    ]
)
