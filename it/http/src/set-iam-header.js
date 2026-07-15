const userId = request.environment.get("Security.DefaultUserId");
console.log("Setting IAM header to: ", userId);
client.global.set("SEC_IAM_HEADER", JSON.stringify(userId));


