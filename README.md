this is a server that is to be ran to give you access to a server via a IOS app that is being developed. 

**Building** 
1. there is a exampledotenv file, simply copy this file and create your own env -> 
```
cp exampledotenv .env
```

fill in the missing information, //TODO - go further into these details.

2. through research, the most easiest and simpliest setup while being free is using Tailscale to gain access and open a port to send HTTPS requests.
download Tailscale on your device and download their CLI tool. 
we need to generate certs for HTTPS access.
```
tailscale cert <tailscale machine name> 
```

```
```
