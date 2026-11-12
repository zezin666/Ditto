# Shared QA isolation suffix helpers.
#
# The core fails closed when DITTO_PIPE_SUFFIX/DITTO_INSTANCE_ID is present but
# invalid. QA scripts must never silently fall back to the production pipe and
# single-instance names, so every script validates the suffix before spawning
# Ditto.

$script:DittoMaxSuffixLength = 48

function Assert-DittoPipeSuffix {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Suffix)

    if ([string]::IsNullOrWhiteSpace($Suffix)) {
        throw "QA pipe suffix is empty; refusing to use the production instance names"
    }
    if ($Suffix.Length -gt $script:DittoMaxSuffixLength) {
        throw "QA pipe suffix is $($Suffix.Length) characters; the maximum is $($script:DittoMaxSuffixLength)"
    }
    if ($Suffix -notmatch '^[A-Za-z0-9_-]+$') {
        throw "QA pipe suffix '$Suffix' contains characters outside [A-Za-z0-9_-]"
    }
    return $Suffix
}

function New-DittoPipeSuffix {
    return [Guid]::NewGuid().ToString("N").Substring(0, 10)
}
