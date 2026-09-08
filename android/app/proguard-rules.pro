# Warren engine JNI (warren-jni/src/android_jni.rs)
#
# The Rust side reaches Java through two surfaces only:
#   1. its own native methods on com.warrenbrowse.vpn.jni.WarrenJni, bound by
#      symbol name (Java_com_warrenbrowse_vpn_jni_WarrenJni_<method>), so the
#      class and its native method names must survive R8. The default
#      proguard-android-optimize.txt already keeps native members by name;
#      naming the class here pins that contract to the one class it protects.
#   2. VpnService.protect(int) on the service object handed to connectTunnel,
#      a framework method that R8 never touches.
# No Kotlin class is looked up by name from Rust (no FindClass), so nothing
# else needs a keep rule for JNI.
-keepclasseswithmembernames class com.warrenbrowse.vpn.jni.WarrenJni {
    native <methods>;
}

# datastore: protobuf lite resolves the message fields by name at runtime.
-keep class com.warrenbrowse.vpn.repository.UserPreferences { *; }
