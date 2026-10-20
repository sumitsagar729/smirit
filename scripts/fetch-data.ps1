# Downloads the offline geodata into ./data:
#   GeoNames (CC BY 4.0): towns of 15,000+ people, state and country names
#   Natural Earth (public domain): country and state outlines for the map
$ErrorActionPreference = 'Stop'
$dir = Join-Path $PSScriptRoot '..\data'
New-Item -ItemType Directory -Force $dir | Out-Null

$files = @(
  @{ name = 'admin1CodesASCII.txt'; url = 'https://download.geonames.org/export/dump/admin1CodesASCII.txt' },
  @{ name = 'countryInfo.txt'; url = 'https://download.geonames.org/export/dump/countryInfo.txt' },
  @{ name = 'ne_50m_admin_0_countries.geojson'; url = 'https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_50m_admin_0_countries.geojson' },
  @{ name = 'ne_50m_admin_1_states_provinces.geojson'; url = 'https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_50m_admin_1_states_provinces.geojson' }
)
foreach ($f in $files) {
  $path = Join-Path $dir $f.name
  if (-not (Test-Path $path)) {
    Write-Host "Downloading $($f.name)"
    Invoke-WebRequest $f.url -OutFile $path
  }
}

$cities = Join-Path $dir 'cities15000.txt'
if (-not (Test-Path $cities)) {
  Write-Host 'Downloading cities15000.zip'
  $zip = Join-Path $dir 'cities15000.zip'
  Invoke-WebRequest 'https://download.geonames.org/export/dump/cities15000.zip' -OutFile $zip
  Expand-Archive $zip -DestinationPath $dir -Force
  Remove-Item $zip
}

# GeoNames updates daily, so check shape rather than checksums.
if ((Get-Content $cities -TotalCount 1).Split("`t").Count -lt 19) { throw 'cities15000.txt looks wrong' }
Write-Host 'Geodata ready.'
