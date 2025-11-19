targetScope = 'subscription'

@description('The environment for the deployment.')
param environment 'prod' | 'test' | 'dev'

@description('The address prefix for the virtual network. Contact Azure owner for your organization to avoid conflicts.')
param addressPrefix string

@description('The container image version to deploy to the container app.')
param imageVersion string

@description('Tags to apply to the resource group for management and organization.')
param tags object = {}

@description('The location for all resources. Should be left to default since it\'s the deployment that decides the location.')
param location string = deployment().location

var workload string = 'joel-bot'
var owner string = 'petter.salminen@magello.se'

var defaultTags = union(tags, {
  workload: workload
  environment: environment
  owner: owner
  slack: '#joel-bot-publik'
})

resource resourceGroup 'Microsoft.Resources/resourceGroups@2025-04-01' = {
  name: 'rg-${workload}-${environment}'
  location: location
  tags: defaultTags
}

module network 'br:crmagello.azurecr.io/bicep/spoke-vnet:latest' = {
  name: 'DeployNetwork'
  scope: resourceGroup
  params: {
    workload: workload
    environment: environment
    addressPrefixes: [
      addressPrefix
    ]
    subnets: [
      {
        name: 'private-endpoints'
        addressPrefix: cidrSubnet(addressPrefix, 27, 0)
      }
    ]
  }
}

var privateEndpointSubnet = last(filter(
  network.outputs.vnet.subnets,
  subnet => contains(subnet.name, 'private-endpoints')
))!

module kv 'br:crmagello.azurecr.io/bicep/keyvault:latest' = {
  name: 'DeployKeyVault'
  scope: resourceGroup
  params: {
    workload: workload
    environment: environment
    privateEndpointSubnetId: privateEndpointSubnet.id
    allowContainerAppEnvironmentAccess: true
  }
}

module ca 'br:crmagello.azurecr.io/bicep/container-app:latest' = {
  name: 'DeployContainerApp'
  scope: resourceGroup
  params: {
    workload: workload
    environment: environment
    location: location
    deployment: {
      type: 'containerApp'
      image: 'crmagello.azurecr.io/joel-bot:${imageVersion}'
      cpu: '0.25'
      memory: '0.5Gi'
      environment: [
        {
          name: 'APP_ENVIRONMENT'
          value: environment
        }
        {
          name: 'JOEL_BOT_SLACK_TOKEN'
          secretRef: 'joelbotslacktoken' 
        }
      ]
      args: [
        '--operation=api'
      ]
    }
    ingress: {
      external: true
      targetPort: 8080
      customDomain: environment == 'prod' ? 'joel-bot.magello.se' : 'joel-bot-${environment}.magello.se'
    }
    secrets: [
      {
        name: 'joelbotslacktoken'
        keyVaultId: kv.outputs.keyVaultId
        secretName: 'joelbotslacktoken'
      }
    ]
  }
}

module caj 'br:crmagello.azurecr.io/bicep/container-job:latest' = {
  name: 'DeployContainerAppJob'
  scope: resourceGroup
  params: {
    workload: workload
    environment: environment
    location: location
    deployment: {
      image: 'crmagello.azurecr.io/joel-bot:${imageVersion}'
      cpu: '0.25'
      memory: '0.5Gi'
      environment: [
        {
          name: 'APP_ENVIRONMENT'
          value: environment
        }
        {
          name: 'JOEL_BOT_SLACK_TOKEN'
          secretRef: 'joelbotslacktoken' 
        }
      ]
      args: [
        '--operation=check_last_workday'
      ]
      trigger: {
        type: 'Schedule'
        schedule: {
          cronExpression: '0 9 * * 1-5' // At 09:00, Monday through Friday
        }
      }
    }
    secrets: [
      {
        name: 'joelbotslacktoken'
        keyVaultId: kv.outputs.keyVaultId
        secretName: 'joelbotslacktoken'
      }
    ]
  }
}
