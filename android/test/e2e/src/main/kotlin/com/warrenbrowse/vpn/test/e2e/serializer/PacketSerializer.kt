package com.warrenbrowse.vpn.test.e2e.serializer

import kotlinx.serialization.KSerializer
import kotlinx.serialization.json.JsonContentPolymorphicSerializer
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import com.warrenbrowse.vpn.test.e2e.router.packetCapture.Packet
import com.warrenbrowse.vpn.test.e2e.router.packetCapture.RxPacket
import com.warrenbrowse.vpn.test.e2e.router.packetCapture.TxPacket

object PacketSerializer : JsonContentPolymorphicSerializer<Packet>(Packet::class) {
    override fun selectDeserializer(element: JsonElement): KSerializer<out Packet> {
        return if (element.jsonObject["from_peer"]?.jsonPrimitive?.booleanOrNull!!) {
            TxPacket.serializer()
        } else {
            RxPacket.serializer()
        }
    }
}
