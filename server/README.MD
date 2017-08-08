How to setup build with tests
========================

1. Get a Microsoft SQL Server somewhere. SQL Express is OK, AzureSQL is OK, LocalDB does **NOT** work.
    * if you want to get SQL Express installed on the default instance (meaning you will not have to include the instance name), you can use this [Chocolatey](chocolatey.org) command `choco install sql-server-express -ia "/IACCEPTSQLSERVERLICENSETERMS /Q /ACTION=install /INSTANCEID=MSSQLSERVER /INSTANCENAME=MSSQLSERVER /UPDATEENABLED=FALSE" -o -y`
2. Configure the crates