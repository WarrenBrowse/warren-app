package com.warrenbrowse.vpn.lib.model

@JvmInline
value class WebsiteAuthToken private constructor(val value: String) {
    companion object {
        fun fromString(value: String) = WebsiteAuthToken(value)
    }
}
