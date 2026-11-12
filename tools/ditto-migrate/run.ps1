param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $Arguments
)

cargo run --quiet -p ditto-migration --bin ditto-migrate -- @Arguments
exit $LASTEXITCODE
