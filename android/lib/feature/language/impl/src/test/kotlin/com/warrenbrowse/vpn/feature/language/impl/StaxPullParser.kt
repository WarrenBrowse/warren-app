package com.warrenbrowse.vpn.feature.language.impl

import java.io.InputStream
import java.io.Reader
import java.io.StringReader
import javax.xml.stream.XMLInputFactory
import javax.xml.stream.XMLStreamConstants
import javax.xml.stream.XMLStreamException
import javax.xml.stream.XMLStreamReader
import org.xmlpull.v1.XmlPullParser
import org.xmlpull.v1.XmlPullParserException

/**
 * A real pull parser for the JVM test suite, over the JDK's own StAX reader.
 *
 * `org.xmlpull.v1` is part of the Android framework, so the unit-test classpath
 * carries only its stubs and `XmlPullParserFactory.newInstance()` throws. This
 * adapter implements exactly the surface `readLocaleConfigTags` uses, so the
 * reader is driven by real XML parsed by a real parser, and a malformed
 * document surfaces as the `XmlPullParserException` a device would raise.
 * Everything else is out of scope and says so.
 */
internal class StaxPullParser(xml: String) : XmlPullParser {
    private val reader: XMLStreamReader =
        XMLInputFactory.newInstance().createXMLStreamReader(StringReader(xml))

    override fun getEventType(): Int = map(reader.eventType)

    override fun next(): Int =
        try {
            map(reader.next())
        } catch (e: XMLStreamException) {
            throw XmlPullParserException("malformed document: ${e::class.simpleName}")
        }

    override fun getName(): String? =
        when (reader.eventType) {
            XMLStreamConstants.START_ELEMENT,
            XMLStreamConstants.END_ELEMENT -> reader.localName
            else -> null
        }

    /**
     * Exact namespace match, the way a device's parser answers: StAX treats a
     * null namespace as "do not check", which would hide the very mistake this
     * test exists to catch.
     */
    override fun getAttributeValue(namespace: String?, name: String?): String? {
        if (reader.eventType != XMLStreamConstants.START_ELEMENT) {
            return null
        }
        for (index in 0 until reader.attributeCount) {
            val attributeNamespace = reader.getAttributeNamespace(index) ?: ""
            if (
                reader.getAttributeLocalName(index) == name &&
                    attributeNamespace == (namespace ?: "")
            ) {
                return reader.getAttributeValue(index)
            }
        }
        return null
    }

    private fun map(staxEvent: Int): Int =
        when (staxEvent) {
            XMLStreamConstants.START_DOCUMENT -> XmlPullParser.START_DOCUMENT
            XMLStreamConstants.END_DOCUMENT -> XmlPullParser.END_DOCUMENT
            XMLStreamConstants.START_ELEMENT -> XmlPullParser.START_TAG
            XMLStreamConstants.END_ELEMENT -> XmlPullParser.END_TAG
            else -> XmlPullParser.TEXT
        }

    private fun unused(): Nothing = error("not used by readLocaleConfigTags")

    override fun setFeature(name: String?, state: Boolean) = unused()

    override fun getFeature(name: String?): Boolean = unused()

    override fun setProperty(name: String?, value: Any?) = unused()

    override fun getProperty(name: String?): Any? = unused()

    override fun setInput(input: Reader?) = unused()

    override fun setInput(inputStream: InputStream?, inputEncoding: String?) = unused()

    override fun getInputEncoding(): String? = unused()

    override fun defineEntityReplacementText(entityName: String?, replacementText: String?) =
        unused()

    override fun getNamespaceCount(depth: Int): Int = unused()

    override fun getNamespacePrefix(pos: Int): String? = unused()

    override fun getNamespaceUri(pos: Int): String? = unused()

    override fun getNamespace(prefix: String?): String? = unused()

    override fun getDepth(): Int = unused()

    override fun getPositionDescription(): String? = unused()

    override fun getLineNumber(): Int = unused()

    override fun getColumnNumber(): Int = unused()

    override fun isWhitespace(): Boolean = unused()

    override fun getText(): String? = unused()

    override fun getTextCharacters(holderForStartAndLength: IntArray?): CharArray? = unused()

    override fun getNamespace(): String? = unused()

    override fun getPrefix(): String? = unused()

    override fun isEmptyElementTag(): Boolean = unused()

    override fun getAttributeCount(): Int = unused()

    override fun getAttributeNamespace(index: Int): String? = unused()

    override fun getAttributeName(index: Int): String? = unused()

    override fun getAttributePrefix(index: Int): String? = unused()

    override fun getAttributeType(index: Int): String? = unused()

    override fun isAttributeDefault(index: Int): Boolean = unused()

    override fun getAttributeValue(index: Int): String? = unused()

    override fun nextToken(): Int = unused()

    override fun require(type: Int, namespace: String?, name: String?) = unused()

    override fun nextText(): String? = unused()

    override fun nextTag(): Int = unused()
}
