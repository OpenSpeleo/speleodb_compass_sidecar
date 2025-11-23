# Log File Location

The application logs are written to:
```
~/.speleodb_compass/speleodb_compass.log
```

## View Logs

To see the logs in real-time:
```bash
tail -f ~/.speleodb_compass/speleodb_compass.log
```

To search for specific entries:
```bash
grep "Downloading project" ~/.speleodb_compass/speleodb_compass.log
grep "404" ~/.speleodb_compass/speleodb_compass.log
```

## What You'll See

The download command logs:
```
Downloading project ZIP from: https://www.speleodb.org/api/v1/projects/{project_id}/download/compass_zip/
```

This will show you the exact URL being requested.

## Debugging the 404

Now with the updated code, the error message in the UI will also show the URL:
```
Download failed with status 404 (URL: https://...)
```

This will help identify if:
1. The endpoint path is wrong
2. The project ID is incorrect
3. The API structure is different than expected
