package com.warrenbrowse.vpn.feature.anticensorship.impl.customport

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextDirection
import com.warrenbrowse.vpn.lib.ui.component.textfield.CustomTextField
import com.warrenbrowse.vpn.lib.ui.component.textfield.ErrorSupportingText
import com.warrenbrowse.vpn.lib.ui.resource.R

@Composable
fun CustomPortTextField(
    value: String,
    modifier: Modifier = Modifier,
    onSubmit: (String) -> Unit,
    onValueChanged: (String) -> Unit,
    isValidValue: Boolean,
    maxCharLength: Int,
    errorText: String?,
) {
    CustomTextField(
        value = value,
        keyboardType = KeyboardType.Number,
        modifier = modifier,
        onValueChanged = onValueChanged,
        onSubmit = onSubmit,
        placeholder = {
            Text(
                stringResource(id = R.string.custom_port_dialog_placeholder),
                style = MaterialTheme.typography.titleMedium,
            )
        },
        maxCharLength = maxCharLength,
        isValidValue = isValidValue,
        isDigitsOnlyAllowed = true,
        textStyle = MaterialTheme.typography.titleMedium.copy(textDirection = TextDirection.Ltr),
        supportingText = errorText?.let { { ErrorSupportingText(errorText) } },
    )
}
