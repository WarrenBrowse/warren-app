# Warren engine JNI (warren-jni/src/android_jni.rs)
#
# The Rust side reaches Java through three surfaces:
#   1. its own native methods on com.warrenbrowse.vpn.jni.WarrenJni, bound by
#      symbol name (Java_com_warrenbrowse_vpn_jni_WarrenJni_<method>);
#   2. VpnService.protect(int) on the service object handed to connectTunnel,
#      a framework method R8 never touches;
#   3. the two methods of the FlowOwnerResolver handed to setFlowOwnerResolver
#      ("Country per app"), called by name: kept by the rule at the bottom.
# No Kotlin class is looked up by name from Rust (no FindClass).
#
# The rule below spells that contract out for this one class; the default
# proguard-android-optimize.txt already applies the same rule to every class.
# It is a keep-names rule, so R8 still drops a native the Kotlin side never
# calls (today four of the 39 declared: productAnchorsJson, importMnemonic,
# signRequest, signCanonicalRequest) and only guarantees that a surviving one
# keeps the exact name its symbol binds to.
-keepclasseswithmembernames class com.warrenbrowse.vpn.jni.WarrenJni {
    native <methods>;
}

# datastore: protobuf lite resolves the message fields by name at runtime.
-keep class com.warrenbrowse.vpn.repository.UserPreferences { *; }

# "Country per app": warren-jni (android_app_routes.rs) calls these two by
# name and signature on the object Kotlin registers. Nothing in Kotlin calls
# them, so without this R8 would drop them and every flow would go
# unattributed.
-keep class com.warrenbrowse.vpn.app.service.FlowOwnerResolver {
    int ownerUid(int, byte[], int, byte[], int);
    java.lang.String[] packagesForUid(int);
}
