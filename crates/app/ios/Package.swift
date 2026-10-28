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
        .binaryTarget(name: "VLCKit", url: "https://download.videolan.org/cocoapods/unstable/VLCKit-4.0-20260929-1631.zip", checksum: "f37c8dbdd4427d1a3f5d75dc4b8bd1ae863cef4426f9a405a2fd1c5c9012f1fb"),
        .target(name: "MediaManagerAds", dependencies: [
            "VLCKit",
            .product(name: "GoogleMobileAds", package: "swift-package-manager-google-mobile-ads"),
            .product(name: "GoogleUserMessagingPlatform", package: "swift-package-manager-google-user-messaging-platform")
        ], linkerSettings: [.unsafeFlags(["-Xlinker", "-ObjC"])])
    ]
)
