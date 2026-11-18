# Tauri invokes this for application binaries, bundled DLLs and installers.
param(
    [Parameter(Mandatory, Position = 0)]
    [string]$Path
)

$ErrorActionPreference = 'Stop'
# Native Azure CLI failures are checked explicitly, including token renewal.
$PSNativeCommandUseErrorActionPreference = $false
foreach ($name in @('AZURE_SIGNING_ENDPOINT', 'AZURE_SIGNING_ACCOUNT', 'AZURE_SIGNING_PROFILE', 'ARTIFACT_SIGNING_MODULE_VERSION')) {
    if ([string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable($name))) {
        throw "Missing $name"
    }
}
if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    throw "Signing input does not exist: $Path"
}
$Path = (Resolve-Path -LiteralPath $Path).Path

Import-Module ArtifactSigning -RequiredVersion $env:ARTIFACT_SIGNING_MODULE_VERSION -ErrorAction Stop

# azure/login's GitHub assertion can expire while Cargo compiles. Reuse the
# signing access token when possible; renew the assertion at the point of use.
az account get-access-token --resource https://codesigning.azure.net --output none --only-show-errors 2>$null
if ($LASTEXITCODE -ne 0) {
    foreach ($name in @('AZURE_CLIENT_ID', 'AZURE_TENANT_ID', 'AZURE_SUBSCRIPTION_ID', 'ACTIONS_ID_TOKEN_REQUEST_URL', 'ACTIONS_ID_TOKEN_REQUEST_TOKEN')) {
        if ([string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable($name))) {
            throw "Missing $name for Azure signing session renewal"
        }
    }
    $separator = if ($env:ACTIONS_ID_TOKEN_REQUEST_URL.Contains('?')) { '&' } else { '?' }
    $audience = [Uri]::EscapeDataString('api://AzureADTokenExchange')
    try {
        $response = Invoke-RestMethod -Method Get -TimeoutSec 30 `
            -Uri "$env:ACTIONS_ID_TOKEN_REQUEST_URL${separator}audience=$audience" `
            -Headers @{ Authorization = "bearer $env:ACTIONS_ID_TOKEN_REQUEST_TOKEN" }
    } catch {
        throw 'Unable to request a fresh GitHub OIDC assertion for Windows signing'
    }
    if ([string]::IsNullOrWhiteSpace($response.value)) {
        throw 'GitHub returned an empty OIDC assertion for Windows signing'
    }
    try {
        az login --service-principal --username $env:AZURE_CLIENT_ID --tenant $env:AZURE_TENANT_ID `
            --federated-token $response.value --output none --only-show-errors *> $null
        if ($LASTEXITCODE -ne 0) { throw 'Azure signing session renewal failed' }
    } catch {
        # Tauri captures and prefixes stdout, so an add-mask command cannot
        # safely protect an assertion printed by the login command here.
        throw 'Azure signing session renewal failed'
    } finally {
        $response = $null
    }
    az account set --subscription $env:AZURE_SUBSCRIPTION_ID --output none --only-show-errors
    if ($LASTEXITCODE -ne 0) { throw 'Unable to select the Azure signing subscription' }
    az account get-access-token --resource https://codesigning.azure.net --output none --only-show-errors *> $null
    if ($LASTEXITCODE -ne 0) { throw 'Unable to acquire an Azure Artifact Signing access token' }
}

Invoke-ArtifactSigning `
    -Endpoint $env:AZURE_SIGNING_ENDPOINT `
    -CodeSigningAccountName $env:AZURE_SIGNING_ACCOUNT `
    -CertificateProfileName $env:AZURE_SIGNING_PROFILE `
    -Files $Path `
    -FileDigest SHA256 `
    -TimestampRfc3161 'http://timestamp.acs.microsoft.com' `
    -TimestampDigest SHA256 `
    -Description 'SpeleoDB Compass Sidecar' `
    -ExcludeEnvironmentCredential `
    -ExcludeWorkloadIdentityCredential `
    -ExcludeManagedIdentityCredential `
    -ExcludeSharedTokenCacheCredential `
    -ExcludeVisualStudioCredential `
    -ExcludeVisualStudioCodeCredential `
    -ExcludeAzurePowerShellCredential `
    -ExcludeAzureDeveloperCliCredential `
    -ExcludeInteractiveBrowserCredential

# Fail before Tauri generates updater signatures or uploads a signed artifact.
$signature = Get-AuthenticodeSignature -LiteralPath $Path
if ($signature.Status -ne 'Valid' -or -not $signature.TimeStamperCertificate) {
    throw "Expected a valid timestamped Authenticode signature: $Path"
}
