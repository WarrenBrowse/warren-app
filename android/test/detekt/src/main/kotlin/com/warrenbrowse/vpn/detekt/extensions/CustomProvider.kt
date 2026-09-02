package com.warrenbrowse.vpn.detekt.extensions

import com.warrenbrowse.vpn.detekt.extensions.rules.ScreenAndDialogNamedArguments
import io.gitlab.arturbosch.detekt.api.Config
import io.gitlab.arturbosch.detekt.api.RuleSet
import io.gitlab.arturbosch.detekt.api.RuleSetProvider

class CustomProvider : RuleSetProvider {

    override val ruleSetId: String = "custom"

    override fun instance(config: Config): RuleSet =
        RuleSet(ruleSetId, listOf(ScreenAndDialogNamedArguments(config)))
}
