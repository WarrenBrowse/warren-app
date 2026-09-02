package com.warrenbrowse.vpn.feature.settings.impl.support

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.unlessIsDetail
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.lib.model.forum.normalizeForumSignInCode
import com.warrenbrowse.vpn.lib.repository.ForumSignInRequests
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import org.koin.compose.koinInject

/**
 * The forum sign-in finished by hand: the approval page shows its session id
 * as a code when tapping its button did not open the app (a browser that
 * asks first, no handler, an old install). Typing it here raises the very same
 * consent prompt a deep link would, so the browser stops being a single point
 * of failure between the forum and the wallet.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ForumSignInCode(navigator: Navigator) {
    val requests = koinInject<ForumSignInRequests>()
    var input by remember { mutableStateOf("") }
    var invalid by remember { mutableStateOf(false) }
    val submit = {
        val sid = normalizeForumSignInCode(input)
        if (sid == null) {
            invalid = true
        } else {
            requests.requestSignIn(sid)
            navigator.goBack()
        }
    }

    ScaffoldWithSmallTopBar(
        appBarTitle = stringResource(R.string.forum_sign_in_code),
        navigationIcon = {
            unlessIsDetail {
                NavigateBackIconButton(onNavigateBack = dropUnlessResumed { navigator.goBack() })
            }
        },
    ) { modifier ->
        Column(
            modifier =
                Modifier.fillMaxSize()
                    .then(modifier)
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = Dimens.sideMargin, vertical = Dimens.mediumPadding),
            verticalArrangement = Arrangement.spacedBy(Dimens.mediumPadding),
        ) {
            Text(
                text = stringResource(R.string.forum_sign_in_code_intro),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            OutlinedTextField(
                value = input,
                onValueChange = {
                    input = it
                    invalid = false
                },
                modifier = Modifier.fillMaxWidth(),
                label = { Text(stringResource(R.string.forum_sign_in_code_label)) },
                placeholder = { Text("0123456789abcdef0123456789abcdef") },
                textStyle = MaterialTheme.typography.bodyLarge.copy(fontFamily = FontFamily.Monospace),
                isError = invalid,
                supportingText =
                    if (invalid) {
                        { Text(stringResource(R.string.forum_sign_in_code_invalid)) }
                    } else null,
                singleLine = true,
                keyboardOptions =
                    KeyboardOptions(
                        capitalization = KeyboardCapitalization.None,
                        autoCorrectEnabled = false,
                        keyboardType = KeyboardType.Ascii,
                        imeAction = ImeAction.Done,
                    ),
                keyboardActions = KeyboardActions(onDone = { submit() }),
            )
            PrimaryButton(
                text = stringResource(R.string.forum_sign_in_code_continue),
                onClick = submit,
                isEnabled = input.isNotBlank(),
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}
